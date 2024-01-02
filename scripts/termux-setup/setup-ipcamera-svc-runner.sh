#!/data/data/com.termux/files/usr/bin/sh
export PKG=ipcamera_bot

cat > $SVDIR/$PKG/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
cd /data/data/com.termux/files/home/Projects/$PKG
exec ./target/release/$PKG 2>&1
EOL
chmod +x $SVDIR/$PKG/run

