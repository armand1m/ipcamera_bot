# ipcamera_bot

This is a telegram bot that fetches a live record from an IP Camera using RTSP protocol, motivated by me going on trips and wanting to keep an eye on my cats.

Written in Rust, made to be deployed locally in your network using an old Android phone through Termux.

## Commands available

- [x] `/get_live`: retrieves 5 seconds of live record from an IP Camera using an RTSP protocol.
    - [x] Camera for this command must be setup through the `CAMERA_URL` _(rtsp stream url)_, `CAMERA_USERNAME` and `CAMERA_PASSWORD` envrionment variables.
    - [x] This command can be renamed with the `GET_RECORD_COMMAND` environment variable (default: `/get_live`)
    - [x] This command can fetch different durations with the `RECORD_DURATION_SECONDS` environment variable (default: 5)
    - [x] This command can fetch a recording without video using the `RECORD_NO_VIDEO` environment variable set to `true` (default: `false`)
    - [x] This command can fetch a recording without audio using the `RECORD_NO_AUDIO` environment variable set to `true` (default: `true`)

You may also send these commands directly to the bot instead of adding it to a chat.

## Running it locally

Environment:

 - Make sure you have Rust installed and updated in your laptop.
 - Make sure you have your IP Camera on the same network as your laptop. _(tip: you can use VLC to test it out)_

Setup:

 - Clone this repository: `git clone https://github.com/armand1m/ipcamera_bot`
 - Make your `.env.`: `cp .env-example .env`
 - Edit the `.env` accordingly. Setup your bot father token, ip camera url, username, password, and properties.

 Now you should be good to get started with.

 - For development: `cargo run`
 - For production:

    ```sh
    cargo build --release
    ./target/release/ipcamera_bot
    ```

## Running with Docker

**Only works on Linux, because host networking in Docker for Mac cannot make this work.**

Setup:

 - Clone this repository: `git clone https://github.com/armand1m/ipcamera_bot`
 - Make your `.env.`: `cp .env-example .env`
 - Edit the `.env` accordingly. Setup your bot father token, ip camera url, username, password, and properties.
 - Start app using docker-compose:
    ```sh
    docker-compose up
    ```

## Running it on termux

I made this bot to be run on an old Android phone through Termux. This is how you can get it setup.

### Setup Termux

Install Termux as recommended in their official API. I would recommend using F-Droid, as it is pretty easy.
Make sure to have these packages installed through F-Droid:

    - Termux
    - Termux API

### Setup runit (through `termux-services`) and SSH

Access termux and install `termux-services`, `git` and `openssh`

```sh
pkg install termux-services 
```

### Setup openssh 

You probably want to `ssh` into your phone from your laptop instead of typing all of this in your phone. _(you could be using a bluetooth keyboard, but you know what I mean, don't do this)_

Install `openssh`:

```sh
pkg install openssh
```

To start the `openssh` service:

```sh
# enable sshd
sv-enable sshd

# start sshd
sv up sshd

# check sshd status
sv status sshd
```

You can check for service logs like this:

```sh
tail -f $LOGDIR/sv/sshd/current
```

`sshd` in termux comes with password authentication enabled by default _(you can check with `cat $PREFIX/etc/ssh/sshd_config`)_,
so if everything is ok, you can run `passwd` to setup a password.

`sshd` will be running on port `8022`. You can connect in your computer using the following:

```sh
ssh root@<phone-ip> -p 8022
```

Use the password you've setup with `passwd`. You can get the ip with `ifconfig`.

## Setup ipcamera_bot environment

Make sure you have `git` and `rust` installed on your termux:

```sh
pkg install git
pkg install rust
```

You might also want `vi`:

```sh
pkg install vi
```

Now we should be good to clone and build the project:

 - Create a folder `~/Projects` and `cd` into it: `mkdir ~/Projects && cd ~/Projects`
 - Clone this repository: `git clone https://github.com/armand1m/ipcamera_bot`
 - `cd` into it: `cd ~/Projects/ipcamera_bot`
 - Make your `.env.`: `cp ~/Projects/ipcamera_bot/.env-example ~/Projects/ipcamera_bot/.env`
 - Edit the `.env` accordingly. Setup your bot father token, ip camera url, username, password, and properties.
    - `vi ~/Projects/ipcamera_bot/.env`

Once done, build the project:

```sh
cd ~/Projects/ipcamera_bot
cargo build --release
```

And now you should have a release build in `~/Projects/ipcamera_bot/target/release/ipcamera_bot`

## Setup ipcamera_bot on runit

Run the following script to setup the service folder and logging:

```sh
export PKG=ipcamera_bot

mkdir -p $SVDIR/$PKG/log
cat > $SVDIR/$PKG/log/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
exec svlogd -v -tt "$LOGDIR/sv/ipcamera_bot"
EOL
chmod +x $SVDIR/$PKG/log/run
```

Now run the following script to create the runner script:

```sh
export PKG=ipcamera_bot
cat > $SVDIR/$PKG/run <<EOL
#!/data/data/com.termux/files/usr/bin/sh
cd /data/data/com.termux/files/home/Projects/ipcamera_bot
exec ./target/release/ipcamera_bot 2>&1
EOL
chmod +x $SVDIR/$PKG/run
```

You should be all set to start the service now:

```sh
sv up ipcamera_bot
```

You can check if it is up with:

```sh
sv status ipcamera_bot
# run: ipcamera_bot: (pid 29368) 1428s, normally down; run: log: (pid 5798) 11062s
```

You should be able to read the logs with:

```sh
tail -f $LOGDIR/sv/ipcamera_bot/current
```

`runit` by default sets up log rotation with up to 10 files with max 1mb each.
Once `current` reached the max size, it gets renamed and a new `current` is created.

### Starting up on boot

Check https://wiki.termux.com/wiki/Termux:Boot