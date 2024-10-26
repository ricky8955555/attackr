#!/bin/sh

set -e

cargo build --release

cp target/release/attackr ./
tar czf attackr.tar.gz attackr static templates

rm attackr
