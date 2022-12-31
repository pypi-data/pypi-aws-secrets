#!/usr/bin/env bash

case "$1" in
  *.tar.gz|*.tgz)
    exec gzip -d -q -c "$1"
    ;;
  *.tar.bz2)
    exec tar -xOzf "$1"
    ;;
  *)
    exec unzip -p "$1"
    ;;
esac
