const core = require('@actions/core');
const github = require('@actions/github');
const toml = require('@iarna/toml');
const semver = require('semver');
const fs = require('fs');
const path = require("path");

try {
    const cargoTomlFile = path.join(process.env.GITHUB_WORKSPACE, "Cargo.toml");
    const cargoToml = toml.parse(fs.readFileSync(cargoTomlFile).toString());
    const versionInput = core.getInput("version").toLowerCase();
    let version;
    switch (versionInput) {
        case "major":
        case "minor":
        case "patch":
            version = semver.inc(cargoToml.package.version, versionInput);
        default:
            version = semver.parse(versionInput).version;
    }
    cargoToml.package.version = version;
    fs.writeFileSync(cargoTomlFile, toml.stringify(cargoToml));
    core.info(`😎 Updated Cargo.toml version to ${version}`);
    core.setOutput("newVersion", version);
} catch (error) {
    core.setFailed(error.message);
}