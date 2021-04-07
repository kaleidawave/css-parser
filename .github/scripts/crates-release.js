const core = require('@actions/core');
const toml = require('@iarna/toml');
const semver = require('semver');
const fs = require('fs');
const path = require("path");

try {
    const cargoTomlFile = path.join(process.env.GITHUB_WORKSPACE, "Cargo.toml");
    const cargoToml = toml.parse(fs.readFileSync(cargoTomlFile).toString());
    const versionInput = core.getInput("version", {required: true});
    console.log(JSON.stringify({cargoToml, versionInput}));
    let version;
    switch (versionInput.toLowerCase()) {
        case "major":
        case "minor":
        case "patch":
            version = semver.inc(cargoToml.package.version, versionInput);
        default:
            const parsedVersion = semver.parse(versionInput);
            if (parsedVersion === null) {
                throw new Error(`Invalid version: "${versionInput}"`);
            } else {
                version = parsedVersion.version;
            }
    }
    cargoToml.package.version = version;
    fs.writeFileSync(cargoTomlFile, toml.stringify(cargoToml));
    core.info(`😎 Updated Cargo.toml version to ${version}`);
    core.setOutput("newVersion", version);
} catch (error) {
    core.setFailed(error.message);
}