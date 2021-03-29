#!/bin/bash


###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# tools/docker/sandbox/scripts/docker-entrypoint.sh                           #
#                                                                             #
# Handcrafted since 2020 by Centrifugians                                     #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Developer sandbox container's entrypoint script.               #
###############################################################################


set -e


# -----------------------------------------------------------------------------
# VARIABLES DEFINITION
# -----------------------------------------------------------------------------

# Reset color mode
COLOR_RESET='\033[0m'

# Default background color definition
COLOR_BACK='\033[49m'

# Font formats
COLOR_BOLD='\033[1m'
COLOR_UNDERLINE='\033[4m'

# Foreground colors definition
COLOR_GREEN="\033[38;5;2m$COLOR_BACK"
COLOR_LIGHT_GREEN="\033[38;5;82m$COLOR_BACK"
COLOR_LIGHT_BLUE="\033[38;5;33m$COLOR_BACK"
COLOR_LIGHT_GREY="\033[0;37m$COLOR_BACK"
COLOR_WHITE="\033[0;231m$COLOR_BACK"


# -----------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# -----------------------------------------------------------------------------

# Initialize Cargo registry
#
# When running an ephemereal Docker container (i.e. docker --rm ...), the Cargo
# registry index is loaded each time. So that to avoid such long update time, 
# the Cargo registry index is stored in the '${WORKSPACE_PATH}/.cargo/registry' folder, on 
# the host's filesystem.
# The ${WORKSPACE_PATH} folder is in fact the mounting point for the Centrifuge chain's code
# stored on the host (see 'Dockerfile' for environment variables definition, such 
# as, for instance, ${WORKSPACE_PATH}).
initialize_local_cargo_registry()
{
    # be sure current directory is a Rust/Cargo project
    if [ ! -e "$WORKSPACE_PATH/Cargo.toml" ]; then
        return
    fi
    
    # check if Cargo's home directory exists, otherwise create it
    if [ ! -d "$CARGO_HOME" ]; then
        mkdir -p "$CARGO_HOME"
    fi

    # check if a project-based Cargo registry exists, and create it otherwise
    if [ ! -d "$PROJECT_CARGO_REGISTRY_PATH" ]; then
        mkdir -p "$PROJECT_CARGO_REGISTRY_PATH"
    fi

    # create a symbolic link between the in-container and the host-based Cargo registry
    if [ ! -L "$CARGO_REGISTRY_PATH" ]; then
        ln -s "$PROJECT_CARGO_REGISTRY_PATH" "$CARGO_REGISTRY_PATH"
    fi

    # five spaces indentation to align on Cargo's log messages format :)
    echo -e "     ${COLOR_GREEN}${COLOR_BOLD}Created${COLOR_RESET} local Cargo registry index/cache in ${COLOR_LIGHT_GREY}${COLOR_BOLD}${PROJECT_CARGO_REGISTRY_PATH}${COLOR_RESET}."
}


# ------------------------------------------------------------------------------
# SCRIPT ENTRY POINT
# ------------------------------------------------------------------------------

# Avoid loading Cargo registry's index each time an ephemereal Docker container 
# is created to run a cargo command, such as, for instance:
#
# $ docker container run --rm -it --volume $PWD:/workspace centrifuge/sandbox cargo build --release
initialize_local_cargo_registry

if [ -z "$1" ]; then
    # No command given at prompt, so let's display container's usage message and launch a bash console
    echo ""
    toilet -f smblock --filter border:metal "           Centrifuge  Chain            "
    echo ""
    echo ""
    echo -e "${COLOR_LIGHT_BLUE}${COLOR_BOLD}${COLOR_UNDERLINE}Tools installed:${COLOR_RESET}"
    echo "  Rust compiler:  $RUST_VERSION"
    echo "  Rust toolchain: $RUST_TOOLCHAIN"
    echo "  Rustup:         $RUSTUP_VERSION"
    echo "  Cargo:          `cargo --version | cut -d' ' -f2`"
    echo ""
    echo -e "${COLOR_LIGHT_BLUE}${COLOR_BOLD}${COLOR_UNDERLINE}Usage:${COLOR_RESET}"
    echo '  docker container run --rm -it --volume=${PWD}:/workspace:rw centrifuge/sandbox:latest [COMMAND] [PARAMETERS]'
    echo ""
    echo -e "${COLOR_LIGHT_BLUE}${COLOR_BOLD}${COLOR_UNDERLINE}NOTE:${COLOR_RESET}"
    echo "  So that to avoid downloading Cargo registry's index and cache each time an ephemereal"
    echo "  Docker container is created, the latter is created in the project's folder"
    echo -e "  on the host (i.e. in '${COLOR_WHITE}${COLOR_BOLD}[workspace_path]/.cargo/registry${COLOR_RESET}' directory) and mounted"
    echo -e "  on the container Cargo registry path, namely '${COLOR_WHITE}${COLOR_BOLD}[cargo_home]/registry${COLOR_RESET}'."
    echo "  As such, the remote registry is downloaded only once, when first running one of"
    echo "  the 'make' commands, such as, for instance, 'make build', to compile and build"
    echo "  the project's excutable."
    echo ""
    echo -e "  ${COLOR_LIGHT_GREEN}Enjoy folks !!!${COLOR_RESET}"
    echo ""

    exec "/bin/bash"
else
    # Execute given command
    exec "$@"
fi
