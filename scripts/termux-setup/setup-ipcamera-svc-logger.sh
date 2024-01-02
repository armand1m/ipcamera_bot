#!/data/data/com.termux/files/usr/bin/sh
export PKG=ipcamera_bot

mkdir -p $LOGDIR/sv/$PKG
mkdir -p $SVDIR/$PKG/log
cat > $SVDIR/$PKG/log/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
exec svlogd -v -tt "$LOGDIR/sv/$PKG"
EOL
chmod +x $SVDIR/$PKG/log/run
