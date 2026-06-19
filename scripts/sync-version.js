#!/usr/bin/env node
/**
 * 同步版本号脚本
 *
 * 从 Cargo.toml (workspace.package.version) 读取版本号，同步到：
 * - ui/package.json
 * - crates/irtool-tauri/tauri.conf.json
 * - crates/irtool-tauri/irtool.manifest (Windows 清单文件)
 * - README.md 版本徽章
 *
 * 用法: node scripts/sync-version.js
 */

const fs = require('fs');
const path = require('path');

const projectRoot = path.resolve(__dirname, '..');

// 读取 Cargo.toml 中的版本号
function getVersionFromCargoToml() {
  const cargoPath = path.join(projectRoot, 'Cargo.toml');
  const content = fs.readFileSync(cargoPath, 'utf-8');
  const match = content.match(/^version\s*=\s*"([^"]+)"/m);
  if (!match) {
    throw new Error('Cannot find version in Cargo.toml');
  }
  return match[1];
}

// 更新 JSON 文件中的版本号
function updateJsonVersion(filePath, version) {
  const content = fs.readFileSync(filePath, 'utf-8');
  const json = JSON.parse(content);
  const oldVersion = json.version;
  json.version = version;
  fs.writeFileSync(filePath, JSON.stringify(json, null, 2) + '\n');
  return oldVersion;
}

// 更新 Windows manifest 文件中的版本号 (x.y.z -> x.y.z.0)
// 只更新 assemblyIdentity 的 version，不碰 XML 声明的 version="1.0"
function updateManifestVersion(filePath, version) {
  const content = fs.readFileSync(filePath, 'utf-8');
  const manifestVersion = version + '.0';
  const oldMatch = content.match(/<assemblyIdentity\s+version="([^"]+)"/);
  const oldVersion = oldMatch ? oldMatch[1] : 'unknown';
  const newContent = content.replace(
    /(<assemblyIdentity\s+version=")[^"]+(")/,
    `$1${manifestVersion}$2`
  );
  fs.writeFileSync(filePath, newContent);
  return oldVersion;
}

// 更新 README.md 中的版本徽章
function updateReadmeBadge(filePath, version) {
  const content = fs.readFileSync(filePath, 'utf-8');
  const oldMatch = content.match(/badge\/v([^"-]+)-blue/);
  const oldVersion = oldMatch ? oldMatch[1] : 'unknown';
  const newContent = content.replace(/badge\/v[^"-]+-blue/, `badge/v${version}-blue`);
  fs.writeFileSync(filePath, newContent);
  return oldVersion;
}

// 主函数
function main() {
  const version = getVersionFromCargoToml();
  console.log(`Version from Cargo.toml: ${version}`);

  // 同步到 ui/package.json
  const packageJsonPath = path.join(projectRoot, 'ui', 'package.json');
  const oldPkgVersion = updateJsonVersion(packageJsonPath, version);
  console.log(`ui/package.json: ${oldPkgVersion} -> ${version}`);

  // 同步到 crates/irtool-tauri/tauri.conf.json
  const tauriConfPath = path.join(projectRoot, 'crates', 'irtool-tauri', 'tauri.conf.json');
  const oldTauriVersion = updateJsonVersion(tauriConfPath, version);
  console.log(`crates/irtool-tauri/tauri.conf.json: ${oldTauriVersion} -> ${version}`);

  // 同步到 crates/irtool-tauri/app-manifest.xml (Tauri WindowsAttributes app_manifest)
  const appManifestPath = path.join(projectRoot, 'crates', 'irtool-tauri', 'app-manifest.xml');
  const oldAppManifestVersion = updateManifestVersion(appManifestPath, version);
  console.log(`crates/irtool-tauri/app-manifest.xml: ${oldAppManifestVersion} -> ${version}.0`);

  // 同步到 README.md
  const readmePath = path.join(projectRoot, 'README.md');
  const oldReadmeVersion = updateReadmeBadge(readmePath, version);
  console.log(`README.md badge: v${oldReadmeVersion} -> v${version}`);

  console.log('\nVersion sync complete!');
}

main();
