# RPM Package Building Framework

## Overview
This framework helps building binary RPM packages for Centrifuge standalone chain and parachain. It supports major RPM-based Linux distributions, including `centos`, `fedora` and `redhat`.

It is worth pointing out that this RPM builder is tailored for Rust-based project. As such, it uses the Cargo [`cargo-rpm`](https://crates.io/crates/cargo-rpm) extension behind the scene for building source and binary RPM packages.

The overall usage is to first build a RPM builder Docker image for a given Linux distribution, and then to build
the RPM package inside a Docker container, more or less as follows:

1. Enter `make build-image redhat` for building the Docker image called `centrifugeio/rpmbuilder:redhat-latest`

2. Then run `docker container run --rm -it -v [PROJECT_ROOT]:/home/rpmbuilder centrifugeio/rpmbuilder:redhat-latest build` for building RPM packages in `[PROJECT_ROOT]/target/release/rpm` folder.

## Modifying project RPM specification file

The RPM specification file [`centrifuge-chain.spec`](./centrifuge-chain.spec) is the one to modify if you wanna tweak some options. It is used for building the Centrifuge chain's RPM packages for different Linux distributions. In fact, so that to not suffer from binary incompatibilities and dangling dependencies between RPM-based distros, this RPM builder framwork supports the major RPM-based Linux distributions (see [Building RPM builder Docker images](#Building_rpm_builder_docker_images) paragraph for more information on how to build RPM builder Docker images).

By default, [`cargo-rpm`](https://crates.io/crates/cargo-rpm) crate searches for the RPM specification file in [[PROJECT_ROOT]/.rpm](../../.rpm) folder to find RPM specification file or building RPM package. A symbolic to the project's [`centrifuge-chain.spec`](./centrifuge-chain.spec) exists.

## Building RPM builder Docker images

For building a RPM builder's Docker image for a given Linux distribution, please proceed as follows:

```sh
$ make build-image [centos | fedora | redhat]
```

For instance, the following command builds a RPM builder Docker image for Red Hat distribution:

```sh
$ make build-image redhat
```

The name of resulting Docker images follow the pattern `centrifugeio/rpmbuilder:fedora-[version]`, where version number follows the [semantic versioning](https://semver.org/) notation (namely `major.minor.patch`) or `latest`.

The name and the tag (default is 'latest') of the image can be modified by tweaking the `DOCKER_IMAGE` and `DOCKER_IMAGE_TAG` variables in the [Makefile](./Makefile), respectively.

For more information on various commands and some examples on how to use them, please enter:

```sh
$ make help
```
## Removing RPM builder Docker images

In order to remove a RPM builder Docker image for a given RPM-based Linux distribution, enter the following command:

```sh
$ make remove-image [centos | fedora | redhat]
```

## Building Centrifuge chain RPM package

After building a RPM builder Docker image for Red Hat, for instance, here's how to use it for building a RPM package for Centrifuge chain, inside a Docker container:

```sh
$ docker container run --rm -it --volume `pwd`:/home/rpmbuilder centrifugeio/rpmbuilder:redhat-[version] build \
  --
```

> The `[version]` placeholder must be replaced in commands with the proper Docker image tag version (enter `docker images` to list all local Docker images and discover `centrifugeio/rpmbuilder:[tag]` images).

The RPM builder image contains a [`build`](./scripts/build.sh) script. For more information on various command-line parameters that can be passed to the `build` command, enter the following command:

```sh
$ docker container run --rm -it centrifugeio/rpmbuilder:redhat-[version] build --help
```

For a Fedora RPM builder image, the same command is:

```sh
$ docker container run --rm -it centrifugeio/rpmbuilder:fedora-[version] build --help
```

Of course, you can run a RPM builder container in interactive mode for manual interaction. For doing so, you should mount the Centrifuge chain project on '/home/rpmbuilder' folder (which is the container's working directory) and enter commands manualy, as show below:

```sh
$ docker docker container run --rm -it --volume [PROJECT_ROOT]:/home/rmpbuilder centrifugeio/rpmbuilder:redhat-[version] bash

$ cargo rpm build -v     # build RPM package in [PROJECT_ROOT]/target/release/rpm...
```