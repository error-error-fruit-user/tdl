#!/usr/bin/bash

expect -c '
set fp [open "token.txt" r]
set file_contents [read $fp]
close $fp

spawn jj git push --bookmark main

expect "username: "
send "wojo81\r"
expect "password:"
send "$file_contents\r"
expect eof
'
