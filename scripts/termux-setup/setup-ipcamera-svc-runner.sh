#!/data/data/com.termux/files/usr/bin/sh
export PKG=ipcamera_bot
cat > $SVDIR/$PKG/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
cd /data/data/com.termux/files/home/Projects/ipcamera_bot
exec ./target/release/ipcamera_bot 2>&1
EOL
chmod +x $SVDIR/$PKG/run

