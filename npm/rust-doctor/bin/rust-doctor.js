#!/usr/bin/env node

"use strict";

const { spawnSync } = require("child_process");

const PLATFORM_PACKAGES = {
  "darwin-x64": "@rust-doctor/darwin-x64",
  "darwin-arm64": "@rust-doctor/darwin-arm64",
  "linux-x64": "@rust-doctor/linux-x64",
  "linux-arm64": "@rust-doctor/linux-arm64",
  "win32-x64": "@rust-doctor/win32-x64",
};

function getBinaryPath() {
  const key = `${process.platform}-${process.arch}`;
  const pkg = PLATFORM_PACKAGES[key];
  if (!pkg) {
    throw new Error(
      `rust-doctor: unsupported platform ${key}. Supported: ${Object.keys(PLATFORM_PACKAGES).join(", ")}`
    );
  }

  const binaryName =
    process.platform === "win32" ? "rust-doctor.exe" : "rust-doctor";
  try {
    return require.resolve(`${pkg}/bin/${binaryName}`);
  } catch {
    throw new Error(
      `rust-doctor: could not find native binary for ${key}.\n` +
        `Try reinstalling: npm install ${pkg}\n` +
        `Or install directly: cargo install rust-doctor`
    );
  }
}

const result = spawnSync(getBinaryPath(), process.argv.slice(2), {
  stdio: "inherit",
  env: process.env,
});

if (result.error) {
  console.error(`rust-doctor: failed to execute binary: ${result.error.message}`);
  process.exit(1);
}

process.exit(result.status ?? 1);
