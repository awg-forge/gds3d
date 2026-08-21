use serde_json::Value;

use super::client::{ArchiveKind, ClientDownload, platform};
use super::{CreateFrpTunnel, FrpNode, FrpTunnel, value_string, value_u16};

const PUBLIC_GAME_FLAG: u64 = 1 << 11;

pub(super) async fn account(credential: &str) -> Result<String, String> {
    let value: Value = reqwest::Client::new()
        .get("https://api.natfrp.com/v4/user/info")
        .query(&[("token", credential)])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    Ok(["/name", "/username"]
        .iter()
        .find_map(|path| value.pointer(path).and_then(Value::as_str))
        .unwrap_or("SakuraFRP")
        .to_owned())
}

pub(super) async fn tunnels(credential: &str) -> Result<Vec<FrpTunnel>, String> {
    let client = reqwest::Client::new();
    let value: Value = client
        .get("https://api.natfrp.com/v4/tunnels")
        .query(&[("token", credential)])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let tunnels = value
        .as_array()
        .ok_or("SakuraFRP returned an invalid tunnel list")?;
    let nodes: Value = client
        .get("https://api.natfrp.com/v4/nodes")
        .query(&[("token", credential)])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    Ok(tunnels
        .iter()
        .map(|item| {
            let node_id = value_string(item.get("node"));
            let node = nodes.get(&node_id);
            let host = node
                .and_then(|node| node.get("host"))
                .and_then(Value::as_str);
            let node_name = node
                .and_then(|node| node.get("name"))
                .and_then(Value::as_str)
                .unwrap_or(&node_id)
                .to_owned();
            let remote = item.get("remote").map(|value| value_string(Some(value)));
            FrpTunnel {
                id: value_string(item.get("id")),
                name: item
                    .get("name")
                    .and_then(Value::as_str)
                    .unwrap_or("SakuraFRP tunnel")
                    .to_owned(),
                node: Some(node_name),
                local_port: value_u16(item.get("local_port")),
                remote_endpoint: endpoint(host, remote.as_deref()),
                online: item.get("online").and_then(Value::as_bool).unwrap_or(false),
            }
        })
        .collect())
}

fn endpoint(host: Option<&str>, remote: Option<&str>) -> Option<String> {
    let remote = remote?.trim();
    if remote.is_empty() {
        return None;
    }
    if remote.parse::<u16>().is_err() {
        return Some(remote.to_owned());
    }
    let host = host?.trim();
    if host.is_empty() {
        return None;
    }
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    Some(format!("{host}:{remote}"))
}

pub(super) async fn nodes(credential: &str) -> Result<Vec<FrpNode>, String> {
    let value: Value = reqwest::Client::new()
        .get("https://api.natfrp.com/v4/nodes")
        .query(&[("token", credential)])
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let nodes = value
        .as_object()
        .ok_or("SakuraFRP returned an invalid node list")?;
    Ok(nodes
        .iter()
        .filter(|(_, node)| is_game_node(node))
        .map(|(id, node)| FrpNode {
            id: id.clone(),
            name: node
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or("SakuraFRP node")
                .to_owned(),
            description: node
                .get("description")
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(str::to_owned),
            vip: node
                .get("vip")
                .and_then(|value| {
                    value
                        .as_u64()
                        .or_else(|| value.as_str().and_then(|value| value.parse().ok()))
                })
                .unwrap_or(0)
                > 0,
            allow_port: None,
        })
        .collect())
}

fn is_game_node(node: &Value) -> bool {
    node.get("flag")
        .and_then(Value::as_u64)
        .is_some_and(|flag| flag & PUBLIC_GAME_FLAG != 0)
}

pub(super) async fn create(credential: &str, request: &CreateFrpTunnel) -> Result<(), String> {
    let node_id = request
        .node_id
        .parse::<u64>()
        .map_err(|_| "SakuraFRP returned an invalid node ID".to_owned())?;
    let body = serde_json::json!({
        "node": node_id,
        "name": request.name.trim(),
        "type": "tcp",
        "note": "Created by SeaLantern Connect",
        "extra": "",
        "local_ip": "127.0.0.1",
        "local_port": request.local_port.to_string(),
        "remote": request.remote_port.trim()
    });
    let response = reqwest::Client::new()
        .post("https://api.natfrp.com/v4/tunnels")
        .bearer_auth(credential)
        .json(&body)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.unwrap_or_default();
        return Err(if message.is_empty() {
            status.to_string()
        } else {
            message
        });
    }
    Ok(())
}

pub(super) async fn remove(credential: &str, tunnel_id: &str) -> Result<(), String> {
    let tunnel_id = tunnel_id
        .parse::<u64>()
        .map_err(|_| "SakuraFRP returned an invalid tunnel ID".to_owned())?;
    let response = reqwest::Client::new()
        .post("https://api.natfrp.com/v4/tunnel/delete")
        .bearer_auth(credential)
        .json(&serde_json::json!({ "ids": tunnel_id }))
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        let status = response.status();
        let message = response.text().await.unwrap_or_default();
        return Err(if message.is_empty() {
            status.to_string()
        } else {
            message
        });
    }
    Ok(())
}

pub(super) async fn client() -> Result<ClientDownload, String> {
    let manifest: Value = reqwest::Client::new()
        .get("https://api.natfrp.com/v4/system/clients")
        .send()
        .await
        .map_err(|error| error.to_string())?
        .error_for_status()
        .map_err(|error| error.to_string())?
        .json()
        .await
        .map_err(|error| error.to_string())?;
    let (os, arch) = platform()?;
    let key = format!("{os}_{arch}");
    let client = manifest
        .pointer(&format!("/frpc/archs/{key}"))
        .ok_or("SakuraFRP does not provide a client for the current platform")?;
    Ok(ClientDownload {
        urls: vec![
            client
                .get("url")
                .and_then(Value::as_str)
                .ok_or("SakuraFRP returned no download URL")?
                .to_owned(),
        ],
        archive: ArchiveKind::Raw,
        expected_size: client.get("size").and_then(Value::as_u64),
        expected_md5: client
            .get("hash")
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

#[cfg(test)]
mod tests {
    use super::{endpoint, is_game_node};

    #[test]
    fn filters_nodes() {
        assert!(is_game_node(&serde_json::json!({ "flag": 2092 })));
        assert!(is_game_node(&serde_json::json!({ "flag": 2188 })));
        assert!(is_game_node(&serde_json::json!({ "flag": 2220 })));
        assert!(!is_game_node(&serde_json::json!({ "flag": 46 })));
    }

    #[test]
    fn builds_endpoint() {
        assert_eq!(
            endpoint(Some("node.example.com"), Some("25565")).as_deref(),
            Some("node.example.com:25565")
        );
        assert_eq!(
            endpoint(Some("node.example.com"), Some("custom.example.com:25565")).as_deref(),
            Some("custom.example.com:25565")
        );
        assert_eq!(endpoint(None, Some("25565")), None);
    }
}
