# This file was generated with the following command:
# update-holochain-versions --git-src=revision:holochain-0.0.151 --lair-version-req=~0.2 --output-file=holochain_version.nix
# For usage instructions please visit https://github.com/holochain/holochain-nixpkgs/#readme

{
    url = "https://github.com/perspect3vism/holochain";
    rev = "fc297f466a3035e7d54050f1f3deefa3b2f70374";
    sha256 = "sha256-Z+F+OIHH8aNJ05/uYNVWdEKCadLhAWy0S0/7daJZZDY=";
    cargoLock = {
        outputHashes = {
        };
    };

    binsFilter = [
        "holochain"
        "hc"
        "kitsune-p2p-proxy"
        "kitsune-p2p-tx2-proxy"
    ];


    lair = {
        url = "https://github.com/holochain/lair";
        rev = "lair_keystore_api-v0.2.0";
        sha256 = "sha256-n7nZyZR0Q68Uff7bTSVFtSDLi21CNcyKibOBx55Gasg=";

        binsFilter = [
            "lair-keystore"
        ];


        cargoLock = {
            outputHashes = {
            };
        };
    };
}
