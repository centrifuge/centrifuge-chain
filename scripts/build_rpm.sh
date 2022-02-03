#!/bin/bash

# This sample script allows to build a RPM package for Centrifuge chain.
#
# Usage is:
#
#   ./scripts/build-rpm.sh [centos | fedora | redhat]
#
# Please refer to the '../ci/rpm/README.md' file on how to build a RPM builder Docker
# image for a RPM-based Linux distro. You must build such Docker image before using this
# sample script.

docker container run --rm -it --volume `pwd`:/home/rpmbuilder centrifugeio/rpmbuilder:redhat-latest build
