#!/bin/bash


################################################################################
# Centrifuge Chain                                                             #
# Infrastructure-as-Code                                                       #
#                                                                              #
# Makefile                                                                     #
#                                                                              #
# Handcrafted since 2022 by Centrifuge contributors                            #
# All rights reserved                                                          #
#                                                                              #
#                                                                              #
# Description: RPM builder image's entry point script.                         #
################################################################################


set -e

if [ -z "$1" ]; then
    # No command given at prompt, so let's display container's usage message and launch a bash console
    echo ""
    echo "Welcome to Centrifuge RPM builder"
    echo ""
    echo "USAGE:"
    echo "  docker container run --rm -it --volume=\`pwd\`:/project:rw [RPM_BUILDER_IMAGE_NAME] build [OPTIONS] [specfile]"
    echo ""
    echo "  For more information on how to use the RPM build command, please enter:"
    echo "    docker container run --rm -it [IMAGE_NAME] build --help"
    echo ""
    echo "EXAMPLES:"
    echo "  To start a RPM builder container in interactive mode, inside a bash shell, that's the command:"
    echo "    docker container run --rm -it [IMAGE_NAME] bash"
    echo ""
    echo "Enjoy folks !!!"
    echo ""

    exec "/bin/build --help"
else 
    # Execute the given command
    exec "$@"
fi
