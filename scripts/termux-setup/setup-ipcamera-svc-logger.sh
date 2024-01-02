#!/data/data/com.termux/files/usr/bin/sh
export PKG=ipcamera_bot

mkdir -p $LOGDIR/sv/ipcamera_bot
mkdir -p $SVDIR/$PKG/log
cat > $SVDIR/$PKG/log/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
exec svlogd -v -tt "$LOGDIR/sv/ipcamera_bot"
EOL
chmod +x $SVDIR/$PKG/log/run
