# Setup

1. ssh administrator@10.19.48.4
2. Install docker: `sudo apt install docker.io`
3. Allow remote connections: https://success.docker.com/article/how-do-i-enable-the-remote-api-for-dockerd
4. There was no DNS server configured it seems, so I fixed this: https://medium.com/@faithfulanere/solved-docker-build-could-not-resolve-archive-ubuntu-com-apt-get-fails-to-install-anything-9ea4dfdcdcf2
    a. sudo vim /etc/docker/daemon.json
    b. sudo service docker restart
4. Observe CPU load: `top` and then in top: `1`

# Usage

1. Connect to VPN (ask @miguel to get access)
1. `export DOCKER_HOST=ssh://administrator@10.19.48.4`
1. `export DOCKER_HOST=tcp://10.19.48.4:2376`
1. Reset with `export DOCKER_HOST=`

# Debug DNS resolving issues:

1. https://stackoverflow.com/questions/24991136/docker-build-could-not-resolve-archive-ubuntu-com-apt-get-fails-to-install-a

# Next steps:

1. Try to download image or setup registry?
    a) https://stackoverflow.com/questions/23935141/how-to-copy-docker-images-from-one-host-to-another-without-using-a-repository
1. Create script for easy execution
1. Caching deps by restructuring docker file: https://blog.mgattozzi.dev/caching-rust-docker-builds/

# Sources:

[1] https://www.digitalocean.com/community/tutorials/how-to-use-a-remote-docker-server-to-speed-up-your-workflow


# Benchmarks:

Building GSRPC (Go) from Dockerfile with all images deleted locally on `master` on Nov 13 2019.

Locally: 695.71 real         3.73 user         3.75 sys


# Remarks:
1. Sending the build context slows down the process, it's for GSRPC ~36MB, and takes ~30secs from my machine. Locally it also takes time, about ~10secs.
2. Pulling images is blazing fast on server1