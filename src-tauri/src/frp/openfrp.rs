use base64::Engine;
use base64::engine::general_purpose::{STANDARD, URL_SAFE, URL_SAFE_NO_PAD};
use crypto_box::aead::{Aead, OsRng};
use crypto_box::{Nonce, PublicKey, SalsaBox, SecretKey};
use reqwest::header::{ACCEPT, AUTHORIZATION, HeaderValue};
use serde_json::Value;
use tauri::AppHandle;
use tauri_plugin_opener::OpenerExt;
use tokio::time::{Duration, sleep};

use super::client::{ArchiveKind, ClientDownload, platform};
use super::{CreateFrpTunnel, FrpNode, FrpTunnel, api_message, value_string, value_u16};

const API_URL: &str = "https://api.openfrp.net/frp/api";
const ACCESS_URL: &str = "https://access.openfrp.net/argoAccess";

fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .user_agent(concat!("SeaLantern-Connect/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|error| error.to_string())
}

async fn post(path: &str, credential: &str, body: Option<&Value>) -> Result<Value, String> {
    let authorization = HeaderValue::from_str(credential)
        .map_err(|_| "OpenFRP Authorization contains invalid characters".to_owned())?;
    let client = http_client()?;
    let mut request = client
        .post(format!("{API_URL}/{path}"))
        .header(AUTHORIZATION, authorization)
        .header(ACCEPT, "application/json");
    request = request.json(body.unwrap_or(&Value::Null));
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    let text = response.text().await.map_err(|error| error.to_string())?;
    let value: Value = serde_json::from_str(&text).map_err(|error| {
        format!("OpenFRP returned an invalid response (HTTP {status}): {error}")
    })?;
    if !status.is_success() {
        return Err(format!(
            "OpenFRP request failed (HTTP {status}): {}",
            api_message(&value, "unknown error")
        ));
    }
    Ok(value)
}

pub(super) async fn browser(app: &AppHandle) -> Result<String, String> {
    let secret = SecretKey::generate(&mut OsRng);
    let public = secret.public_key();
    let public_key = URL_SAFE.encode(public.as_bytes());
    let client = http_client()?;
    let response = client
        .post(format!("{ACCESS_URL}/requestLogin"))
        .json(&serde_json::json!({ "public_key": public_key }))
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?;
    let value: Value = response.json().await.map_err(|error| error.to_string())?;
    let url = required_str(&value, "/data/authorization_url")?;
    let request_id = required_str(&value, "/data/request_uuid")?;
    app.opener()
        .open_url(url, None::<&str>)
        .map_err(|error| error.to_string())?;
    poll(&client, request_id, &secret).await
}

async fn poll(
    client: &reqwest::Client,
    request_id: &str,
    secret: &SecretKey,
) -> Result<String, String> {
    for _ in 0..60 {
        sleep(Duration::from_secs(5)).await;
        let response = client
            .get(format!("{ACCESS_URL}/pollLogin"))
            .query(&[("request_uuid", request_id)])
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if !response.status().is_success() {
            continue;
        }
        let server_key = response
            .headers()
            .get("x-request-public-key")
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned);
        let value: Value = response.json().await.map_err(|error| error.to_string())?;
        if value.get("code").and_then(Value::as_u64) != Some(200) {
            continue;
        }
        let server_key = server_key.ok_or("OpenFRP did not return its public key")?;
        let encrypted = required_str(&value, "/data/authorization_data")?;
        return decrypt(&server_key, encrypted, secret);
    }
    Err("OpenFRP browser authorization timed out".to_owned())
}

fn decrypt(server_key: &str, encrypted: &str, secret: &SecretKey) -> Result<String, String> {
    let key = decode_url(server_key)?;
    let key: [u8; 32] = key
        .try_into()
        .map_err(|_| "OpenFRP returned an invalid public key".to_owned())?;
    let encrypted = STANDARD
        .decode(encrypted)
        .map_err(|_| "OpenFRP returned invalid authorization data".to_owned())?;
    if encrypted.len() <= 24 {
        return Err("OpenFRP returned incomplete authorization data".to_owned());
    }
    let cipher = SalsaBox::new(&PublicKey::from(key), secret);
    let nonce = Nonce::from_slice(&encrypted[..24]);
    let decrypted = cipher
        .decrypt(nonce, &encrypted[24..])
        .map_err(|_| "OpenFRP authorization could not be decrypted".to_owned())?;
    String::from_utf8(decrypted).map_err(|_| "OpenFRP returned an invalid Authorization".to_owned())
}

fn decode_url(value: &str) -> Result<Vec<u8>, String> {
    URL_SAFE
        .decode(value)
        .or_else(|_| URL_SAFE_NO_PAD.decode(value))
        .map_err(|_| "OpenFRP returned an invalid public key".to_owned())
}

fn required_str<'a>(value: &'a Value, path: &str) -> Result<&'a str, String> {
    value
        .pointer(path)
        .and_then(Value::as_str)
        .ok_or_else(|| "OpenFRP returned an incomplete authorization response".to_owned())
}

pub(super) async fn account(credential: &str) -> Result<String, String> {
    let value = user_info(credential).await?;
    Ok(["/data/username", "/data/name", "/data/user"]
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_str))
        .unwrap_or("OpenFRP")
        .to_owned())
}

pub(super) async fn token(credential: &str) -> Result<String, String> {
    let value = user_info(credential).await?;
    required_str(&value, "/data/token").map(str::to_owned)
}

async fn user_info(credential: &str) -> Result<Value, String> {
    let value = post("getUserInfo", credential, None).await?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "OpenFRP authorization was rejected"));
    }
    Ok(value)
}

pub(super) async fn tunnels(credential: &str) -> Result<Vec<FrpTunnel>, String> {
    let value = post("getUserProxies", credential, None).await?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "failed to load OpenFRP tunnels"));
    }
    Ok(value
        .pointer("/data/list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .map(|item| FrpTunnel {
            id: value_string(item.get("id")),
            name: item
                .get("proxyName")
                .and_then(Value::as_str)
                .unwrap_or("OpenFRP tunnel")
                .to_owned(),
            node: item
                .get("friendlyNode")
                .or_else(|| item.get("node"))
                .or_else(|| item.get("nodeName"))
                .map(|value| value_string(Some(value))),
            local_port: value_u16(item.get("localPort").or_else(|| item.get("local_port"))),
            remote_endpoint: endpoint(item),
            online: item.get("online").and_then(Value::as_bool).unwrap_or(false),
        })
        .collect())
}

fn endpoint(item: &Value) -> Option<String> {
    item.get("connectAddress")
        .or_else(|| item.get("connect_address"))
        .or_else(|| item.get("remote"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub(super) async fn nodes(credential: &str) -> Result<Vec<FrpNode>, String> {
    let (value, user) =
        tokio::try_join!(post("getNodeList", credential, None), user_info(credential))?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "failed to load OpenFRP nodes"));
    }
    let group = user
        .pointer("/data/group")
        .and_then(Value::as_str)
        .ok_or("OpenFRP returned incomplete account information")?;
    let realname = user
        .pointer("/data/realname")
        .and_then(Value::as_bool)
        .unwrap_or(false);
    Ok(value
        .pointer("/data/list")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter(|node| usable(node, group, realname))
        .map(|node| FrpNode {
            id: value_string(node.get("id")),
            name: node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("OpenFRP node")
                .to_owned(),
            description: node
                .get("description")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            vip: false,
            allow_port: port_range(node),
        })
        .collect())
}

fn usable(node: &Value, group: &str, realname: bool) -> bool {
    value_u16(node.get("status")) == Some(200)
        && node
            .pointer("/protocolSupport/tcp")
            .and_then(Value::as_bool)
            == Some(true)
        && node.get("fullyLoaded").and_then(Value::as_bool) != Some(true)
        && node
            .get("group")
            .and_then(Value::as_str)
            .is_some_and(|groups| groups.split(';').any(|item| item.trim() == group))
        && (realname || node.get("needRealname").and_then(Value::as_bool) != Some(true))
        && address_ok(node.get("hostname"))
        && port_ok(node.get("port"))
}

fn address_ok(value: Option<&Value>) -> bool {
    value
        .and_then(Value::as_str)
        .map(str::trim)
        .is_some_and(|value| !value.is_empty() && !value.contains("无权查询"))
}

fn port_ok(value: Option<&Value>) -> bool {
    value_u16(value).is_some() || address_ok(value)
}

fn port_range(node: &Value) -> Option<String> {
    node.get("allowPort")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub(super) async fn create(credential: &str, request: &CreateFrpTunnel) -> Result<(), String> {
    let node_id = request
        .node_id
        .parse::<u64>()
        .map_err(|_| "OpenFRP returned an invalid node ID".to_owned())?;
    let body = serde_json::json!({
        "node_id": node_id,
        "name": request.name.trim(),
        "type": "tcp",
        "local_addr": "127.0.0.1",
        "local_port": request.local_port.to_string(),
        "remote_port": request.remote_port.trim(),
        "domain_bind": "",
        "dataGzip": true,
        "dataEncrypt": false,
        "url_route": "",
        "host_rewrite": "",
        "request_from": "",
        "request_pass": "",
        "custom": ""
    });
    let value = post("newProxy", credential, Some(&body)).await?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "OpenFRP rejected the tunnel"));
    }
    Ok(())
}

pub(super) async fn edit(
    credential: &str,
    tunnel_id: &str,
    request: &CreateFrpTunnel,
) -> Result<(), String> {
    let node_id = request
        .node_id
        .parse::<u64>()
        .map_err(|_| "OpenFRP returned an invalid node ID".to_owned())?;
    let proxy_id = tunnel_id
        .parse::<u64>()
        .map_err(|_| "OpenFRP returned an invalid tunnel ID".to_owned())?;
    let body = serde_json::json!({
        "proxy_id": proxy_id, "node_id": node_id, "name": request.name.trim(),
        "type": "tcp", "local_addr": "127.0.0.1", "local_port": request.local_port.to_string(),
        "remote_port": request.remote_port.trim(), "domain_bind": "", "custom": "",
        "dataGzip": true, "dataEncrypt": false, "url_route": "", "host_rewrite": "",
        "request_from": "", "request_pass": ""
    });
    let value = post("editProxy", credential, Some(&body)).await?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "OpenFRP rejected the tunnel edit"));
    }
    Ok(())
}

pub(super) async fn remove(credential: &str, tunnel_id: &str) -> Result<(), String> {
    let body = serde_json::json!({ "proxy_id": tunnel_id });
    let value = post("removeProxy", credential, Some(&body)).await?;
    if value.get("flag").and_then(Value::as_bool) != Some(true) {
        return Err(api_message(&value, "OpenFRP rejected the deletion"));
    }
    Ok(())
}

pub(super) async fn client() -> Result<ClientDownload, String> {
    let manifest: Value = http_client()?
        .get("https://api.openfrp.net/commonQuery/get?key=software")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let data = manifest
        .get("data")
        .ok_or("OpenFRP returned an invalid software manifest")?;
    let (os, arch) = platform()?;
    let file = data
        .get("soft")
        .and_then(Value::as_array)
        .and_then(|systems| {
            systems
                .iter()
                .find(|system| system.get("os").and_then(Value::as_str) == Some(os))
        })
        .and_then(|system| system.get("arch"))
        .and_then(Value::as_array)
        .and_then(|architectures| {
            architectures
                .iter()
                .find(|item| item.get("key").and_then(Value::as_str) == Some(arch))
        })
        .and_then(|item| item.get("file"))
        .and_then(Value::as_str)
        .ok_or("OpenFRP does not provide a client for the current platform")?;
    let sources = data
        .get("source")
        .and_then(Value::as_array)
        .ok_or("OpenFRP returned no download source")?;
    let release = data
        .get("latest")
        .and_then(Value::as_str)
        .ok_or("OpenFRP returned no current release")?;
    let archive = if file.ends_with(".zip") {
        ArchiveKind::Zip
    } else if file.ends_with(".tar.gz") {
        ArchiveKind::TarGz
    } else {
        return Err("OpenFRP returned an unsupported client archive".to_owned());
    };
    let urls: Vec<String> = sources
        .iter()
        .filter_map(|source| source.get("value").and_then(Value::as_str))
        .map(|source| format!("{source}{release}{file}"))
        .collect();
    if urls.is_empty() {
        return Err("OpenFRP returned no download source".to_owned());
    }
    Ok(ClientDownload {
        urls,
        archive,
        expected_size: None,
        expected_md5: None,
    })
}

#[cfg(test)]
mod tests {
    use super::{STANDARD, URL_SAFE, decrypt, endpoint, port_range, usable};
    use base64::Engine;
    use crypto_box::aead::Aead;
    use crypto_box::{Nonce, SalsaBox, SecretKey};

    #[test]
    fn decrypts_auth() {
        let client = SecretKey::from([3; 32]);
        let server = SecretKey::from([7; 32]);
        let cipher = SalsaBox::new(&client.public_key(), &server);
        let nonce = Nonce::from_slice(&[11; 24]);
        let encrypted = cipher.encrypt(nonce, b"Bearer test".as_slice()).unwrap();
        let mut payload = nonce.to_vec();
        payload.extend(encrypted);
        let server_key = URL_SAFE.encode(server.public_key().as_bytes());

        assert_eq!(
            decrypt(&server_key, &STANDARD.encode(payload), &client).unwrap(),
            "Bearer test"
        );
    }

    #[test]
    fn reads_endpoint() {
        let tunnel = serde_json::json!({
            "connectAddress": "cn.example.com:25565",
            "remotePort": 25565
        });
        assert_eq!(endpoint(&tunnel).as_deref(), Some("cn.example.com:25565"));
        assert_eq!(endpoint(&serde_json::json!({ "remotePort": 25565 })), None);
    }

    #[test]
    fn filters_nodes() {
        let node = serde_json::json!({
            "status": 200,
            "group": "normal;vip",
            "hostname": "node.example.com",
            "port": 7000,
            "needRealname": false,
            "fullyLoaded": false,
            "protocolSupport": { "tcp": true }
        });
        assert!(usable(&node, "normal", false));
        assert!(!usable(&node, "svip", false));

        let mut loaded = node.clone();
        loaded["fullyLoaded"] = true.into();
        assert!(!usable(&loaded, "normal", false));

        let mut hidden = node;
        hidden["hostname"] = "您无权查询此节点的地址".into();
        assert!(!usable(&hidden, "normal", false));
    }

    #[test]
    fn reads_port_range() {
        let node = serde_json::json!({ "allowPort": "(50000,60000)" });
        assert_eq!(port_range(&node).as_deref(), Some("(50000,60000)"));
        assert_eq!(port_range(&serde_json::json!({ "allowPort": null })), None);
    }
}
