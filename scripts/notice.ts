import { access, readdir, readFile, unlink, writeFile } from "node:fs/promises";
import { exec, execFile } from "node:child_process";
import { homedir } from "node:os";
import path from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

interface FrontendDependency {
  licenseFile?: string;
  licenses?: string;
  publisher?: string;
  repository?: string;
}

interface BackendCrate {
  authors?: string | string[];
  license?: string;
  license_file?: string;
  name: string;
  repository?: string;
  version: string;
}

type FrontendLicenses = Record<string, FrontendDependency>;

const execFileAsync = promisify(execFile);
const execAsync = promisify(exec);
const rootDir = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const cargoDir = path.join(rootDir, "src-tauri");
const temporaryFiles = [
  path.join(rootDir, "frontend-licenses.json"),
  path.join(rootDir, "backend-licenses.json"),
];
const noticeFile = path.join(rootDir, "NOTICE");

async function exists(filePath: string): Promise<boolean> {
  try {
    await access(filePath);
    return true;
  } catch {
    return false;
  }
}

async function run(command: string, args: string[], cwd: string) {
  return execFileAsync(command, args, { cwd, maxBuffer: 16 * 1024 * 1024 });
}

async function ensureCargoLicense() {
  try {
    await run("cargo", ["license", "--help"], cargoDir);
  } catch {
    console.log("cargo-license is not installed; installing it now...");
    await run("cargo", ["install", "cargo-license"], cargoDir);
  }
}

async function frontendLicenses(): Promise<FrontendLicenses> {
  const outputFile = temporaryFiles[0];
  const { stdout } = await execAsync(
    "npx --yes license-checker-rseidelsohn --start . --json --production",
    { cwd: rootDir, maxBuffer: 16 * 1024 * 1024 },
  );
  await writeFile(outputFile, stdout, "utf8");
  return JSON.parse(stdout) as FrontendLicenses;
}

async function backendLicenses(): Promise<BackendCrate[]> {
  await ensureCargoLicense();
  const outputFile = temporaryFiles[1];
  const { stdout } = await run("cargo", ["license", "--json", "--avoid-dev-deps"], cargoDir);
  await writeFile(outputFile, stdout, "utf8");
  return JSON.parse(stdout) as BackendCrate[];
}

async function readFrontendLicense(filePath: string | undefined): Promise<string | null> {
  if (!filePath || /README|\.spdx/i.test(filePath)) return null;
  const candidates = [path.resolve(rootDir, filePath), filePath];
  const licenses = await Promise.all(
    candidates.map(async (candidate) => {
      if (!(await exists(candidate))) return null;
      return (await readFile(candidate, "utf8")).replace(/^\uFEFF/, "").trim();
    }),
  );
  return licenses.find(Boolean) || null;
}

async function readBackendLicense(crate: BackendCrate): Promise<string | null> {
  if (crate.license_file && (await exists(crate.license_file))) {
    return (await readFile(crate.license_file, "utf8")).trim();
  }

  const cargoHome = process.env.CARGO_HOME || path.join(homedir(), ".cargo");
  const registryDir = path.join(cargoHome, "registry", "src");
  if (!(await exists(registryDir))) return null;

  const registries = (await readdir(registryDir, { withFileTypes: true })).filter((entry) =>
    entry.isDirectory(),
  );
  const licenses = await Promise.all(
    registries.map(async (registry) => {
      const crateDir = path.join(registryDir, registry.name, `${crate.name}-${crate.version}`);
      if (!(await exists(crateDir))) return null;
      const license = (await readdir(crateDir, { withFileTypes: true })).find(
        (file) => file.isFile() && /^(license|copying)([.-]|$)/i.test(file.name),
      );
      return license ? (await readFile(path.join(crateDir, license.name), "utf8")).trim() : null;
    }),
  );
  return licenses.find(Boolean) || null;
}

function frontendSection(
  name: string,
  version: string,
  dependency: FrontendDependency,
  id: number,
): string {
  return [
    `${id}. ${name}@${version}`,
    "",
    dependency.publisher ? `Copyright (c) ${dependency.publisher}` : null,
    dependency.repository ? `Source: ${dependency.repository}` : null,
    `License: ${dependency.licenses}`,
    "",
  ]
    .filter((line) => line !== null)
    .join("\n");
}

function backendSection(crate: BackendCrate, id: number): string {
  const authors = Array.isArray(crate.authors) ? crate.authors : [crate.authors].filter(Boolean);
  return [
    `${id}. ${crate.name}@${crate.version}`,
    "",
    authors.length > 0 ? `Authors: ${authors.join(", ")}` : null,
    crate.repository ? `Repository: ${crate.repository}` : null,
    `License: ${crate.license || "Unknown"}`,
    "",
  ]
    .filter((line) => line !== null)
    .join("\n");
}

async function generateNotice(): Promise<void> {
  const packageJson = JSON.parse(await readFile(path.join(rootDir, "package.json"), "utf8")) as {
    name: string;
  };
  const [frontend, backend] = await Promise.all([frontendLicenses(), backendLicenses()]);
  const lines = [
    "gds3d",
    "Copyright (C) 2026 AWG Forge",
    "Licensed under the Apache License, Version 2.0.",
    "",
    "---",
    "",
    "Third-party frontend dependencies:",
    "",
  ];

  const packages = Object.entries(frontend)
    .filter(([name]) => !name.startsWith(`${packageJson.name}@`))
    .toSorted(([left], [right]) => left.localeCompare(right));
  const frontendSections = await Promise.all(
    packages.map(async ([packageKey, dependency], index) => {
      const separator = packageKey.lastIndexOf("@");
      const name = packageKey.slice(0, separator);
      const version = packageKey.slice(separator + 1);
      const license = await readFrontendLicense(dependency.licenseFile);
      return [
        frontendSection(name, version, dependency, index + 1),
        ...(license ? [license, ""] : []),
      ];
    }),
  );
  lines.push(...frontendSections.flat());

  lines.push("", "---", "", "Third-party backend dependencies:", "");
  const crates = backend
    .filter((crate) => crate.name !== "gds3d")
    .toSorted((left, right) =>
      `${left.name}@${left.version}`.localeCompare(`${right.name}@${right.version}`),
    );
  const backendSections = await Promise.all(
    crates.map(async (crate, index) => {
      const license = await readBackendLicense(crate);
      return [backendSection(crate, index + 1), ...(license ? [license, ""] : [])];
    }),
  );
  lines.push(...backendSections.flat());

  await writeFile(noticeFile, lines.join("\n"), "utf8");
  await Promise.all(
    temporaryFiles.map(async (file) => {
      if (await exists(file)) await unlink(file);
    }),
  );
  console.log(`Generated ${noticeFile}`);
}

const [argument] = process.argv.slice(2);
if (argument === "--help" || argument === "-h") {
  console.log("Usage: pnpm notice");
} else if (argument) {
  throw new Error(`Unknown argument: ${argument}`);
} else {
  generateNotice().catch((error: unknown) => {
    console.error(
      `Failed to generate NOTICE: ${error instanceof Error ? error.message : String(error)}`,
    );
    process.exitCode = 1;
  });
}
