import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

type VersionedJson = Record<string, unknown> & { version: string };

interface CargoVersionRange {
  end: number;
  start: number;
  value: string;
}

interface CurrentVersions {
  cargoRange: CargoVersionRange;
  cargoSource: string;
  packageJson: VersionedJson;
  packageSource: string;
  tauriJson: VersionedJson;
  tauriSource: string;
}

type VersionUpdate = [file: string, source: string, previousSource: string];

const scriptDir = path.dirname(fileURLToPath(import.meta.url));
const rootDir = path.resolve(scriptDir, "..");
const files = {
  cargo: path.join(rootDir, "Cargo.toml"),
  package: path.join(rootDir, "package.json"),
  tauri: path.join(rootDir, "src-tauri", "tauri.conf.json"),
};
const semverPattern =
  /^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-((?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*)(?:\.(?:0|[1-9]\d*|\d*[A-Za-z-][0-9A-Za-z-]*))*))?(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?$/;

function cargoVersionRange(source: string): CargoVersionRange {
  const sectionMatch = /^\[workspace\.package\]\s*$/m.exec(source);
  if (!sectionMatch) throw new Error("Cargo.toml is missing [workspace.package]");

  const sectionStart = sectionMatch.index + sectionMatch[0].length;
  const nextSection = /^\[/m.exec(source.slice(sectionStart));
  const sectionEnd = nextSection ? sectionStart + nextSection.index : source.length;
  const section = source.slice(sectionStart, sectionEnd);
  const matches = [...section.matchAll(/^version\s*=\s*"([^"]+)"\s*$/gm)];
  if (matches.length !== 1) {
    throw new Error("Cargo.toml must contain one workspace package version");
  }

  const match = matches[0];
  return {
    end: sectionStart + match.index + match[0].length,
    start: sectionStart + match.index,
    value: match[1],
  };
}

function parseJson(source: string, label: string): VersionedJson {
  let value: unknown;
  try {
    value = JSON.parse(source);
  } catch (error) {
    const message = error instanceof Error ? error.message : String(error);
    throw new Error(`${label} is not valid JSON: ${message}`, { cause: error });
  }
  if (typeof value !== "object" || value === null || typeof value.version !== "string") {
    throw new Error(`${label} is missing a string version`);
  }
  return value as VersionedJson;
}

async function readVersions(): Promise<CurrentVersions> {
  const [cargoSource, packageSource, tauriSource] = await Promise.all([
    readFile(files.cargo, "utf8"),
    readFile(files.package, "utf8"),
    readFile(files.tauri, "utf8"),
  ]);
  const cargoRange = cargoVersionRange(cargoSource);
  const packageJson = parseJson(packageSource, "package.json");
  const tauriJson = parseJson(tauriSource, "src-tauri/tauri.conf.json");
  const versions = [cargoRange.value, packageJson.version, tauriJson.version];
  if (!versions.every((version) => version === versions[0])) {
    throw new Error(
      `version mismatch: Cargo=${versions[0]}, package=${versions[1]}, Tauri=${versions[2]}`,
    );
  }
  return { cargoRange, cargoSource, packageJson, packageSource, tauriJson, tauriSource };
}

async function writeVersions(current: CurrentVersions, version: string): Promise<void> {
  const cargoSource =
    current.cargoSource.slice(0, current.cargoRange.start) +
    `version = "${version}"` +
    current.cargoSource.slice(current.cargoRange.end);
  const packageSource = `${JSON.stringify({ ...current.packageJson, version }, null, 2)}\n`;
  const tauriSource = `${JSON.stringify({ ...current.tauriJson, version }, null, 2)}\n`;
  const updates: VersionUpdate[] = [
    [files.cargo, cargoSource, current.cargoSource],
    [files.package, packageSource, current.packageSource],
    [files.tauri, tauriSource, current.tauriSource],
  ];
  try {
    await Promise.all(updates.map(([file, source]) => writeFile(file, source, "utf8")));
  } catch (error) {
    await Promise.allSettled(updates.map(([file, , source]) => writeFile(file, source, "utf8")));
    throw error;
  }
}

async function main(): Promise<void> {
  const [command, versionTag, ...extra] = process.argv.slice(2);
  if (command === "show" && versionTag === undefined) {
    const current = await readVersions();
    console.log(`v${current.cargoRange.value}`);
    return;
  }
  if (command !== "change" || versionTag === undefined || extra.length !== 0) {
    throw new Error("usage: version.ts show | version.ts change <vMAJOR.MINOR.PATCH>");
  }
  if (!versionTag.startsWith("v")) {
    throw new Error(`version must start with v: ${versionTag}`);
  }
  const version = versionTag.slice(1);
  if (version.length > 128 || !semverPattern.test(version)) {
    throw new Error(`invalid semantic version: ${versionTag}`);
  }

  const current = await readVersions();
  const previous = current.cargoRange.value;
  if (version === previous) {
    console.log(`v${previous}`);
    return;
  }
  await writeVersions(current, version);
  console.log(`v${previous} -> v${version}`);
}

main().catch((error: unknown) => {
  console.error(`version: ${error instanceof Error ? error.message : String(error)}`);
  process.exitCode = 1;
});
