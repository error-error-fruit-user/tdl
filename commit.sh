#!/usr/bin/bash

jj describe -m "$@"
jj bookmark set main -r @
