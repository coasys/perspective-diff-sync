#!/bin/bash
set -e

if ! nix-shell --help &> /dev/null
then
    echo "nix-shell could not be found! Are you sure it is installed correctly?"
    exit
fi

echo "Creating four releases of Social-Context inside ./release"

[ ! -d "./release" ] && mkdir "./release"

echo "Create release with no features enabled..."

#Get new dna.yaml with correct props & build language
npm install && npm run build

#Copy the build files to the release dir
cp ./build/bundle.js ./release/bundle.js
cp ./hc-dna/workdir/perspective-diff-sync.dna ./release/perspective-diff-sync.dna