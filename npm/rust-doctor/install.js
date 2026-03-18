"use strict";

// Postinstall script: verifies the native binary is available.
// The binary is provided by platform-specific optionalDependencies.

const { execFileSync } = require("child_process");

const PLATFORM_PACKAGES = {
  "darwin-x64": "@rust-doctor/darwin-x64",
  "darwin-arm64": "@rust-doctor/darwin-arm64",
  "linux-x64": "@rust-doctor/linux-x64",
  "linux-arm64": "@rust-doctor/linux-arm64",
  "win32-x64": "@rust-doctor/win32-x64",
};

const key = `${process.platform}-${process.arch}`;
const pkg = PLATFORM_PACKAGES[key];

if (!pkg) {
  console.warn(
    `rust-doctor: no pre-built binary for ${key}. ` +
      `Install from source: cargo install rust-doctor`
  );
  process.exit(0);
}

const binaryName =
  process.platform === "win32" ? "rust-doctor.exe" : "rust-doctor";

try {
  const binaryPath = require.resolve(`${pkg}/bin/${binaryName}`);
  execFileSync(binaryPath, ["--version"], { stdio: "pipe" });
  console.log(`rust-doctor: using native binary for ${key}`);
} catch {
  console.warn(
    `rust-doctor: native binary not found for ${key}. ` +
      `The JS shim will attempt to locate it at runtime.`
  );
}
