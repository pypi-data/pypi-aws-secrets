#!/usr/bin/env bash

case "$1" in
  *.tar.gz|*.tgz)
    exec gzip -d -q -c "$1"
    ;;
  *.tar.bz2)
    exec tar -xOzf "$1"
    ;;
  *.gem)
    # Gemfiles are tar files with several individual gzip files inside.
    tar -xOzf "$1" "data.tar.gz" | gzip -d
    ;;
  *.tar)
    # Tar files (from hexpm) contain a single file
    tar -xOzf "$1" "contents.tar.gz" | gzip -d
    ;;
  *)
    exec unzip -p "$1"
    ;;
esac
