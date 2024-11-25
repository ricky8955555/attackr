#!/bin/sh

set -e

if [ "$#" -eq 0 ]; then
    cargo build --release
elif [ "$#" -eq 1 ]; then
    cargo build --release -F $1
else
    echo "too many arguments."
    exit 1
fi

cp target/release/attackr ./
tar czf attackr.tar.gz attackr static templates

rm attackr
