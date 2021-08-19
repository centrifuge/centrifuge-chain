################################################################################
# Centrifuge                                                                   #
# Cash on Steroids                                                             #
#                                                                              #
# tools/testbed/Makefile                                                       #
#                                                                              #
# Handcrafted since 2020 by Centrifuge Foundation                              #
# All rights reserved                                                          #
#                                                                              #
#                                                                              #
# Description: Centrifuge chain's testbed infrastructure management script.    #
################################################################################


# ------------------------------------------------------------------------------
# VARIABLES DEFINITION
# ------------------------------------------------------------------------------

# Colors definition
include ../automake/colors.mk

# Project settings (e.g. Rust toolchain version, ...)
include ./automake/settings.mk


# ------------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# ------------------------------------------------------------------------------

# Display help/usage message
define help
	@echo ""
	@echo "$(COLOR_WHITE)Centrifuge$(COLOR_RESET)"
	@echo "$(COLOR_WHITE)Cash on Steroids$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_BLUE)Testbed$(COLOR_RESET)"
	@echo ""
	@echo "Handcrafted since 2020 by Centrifuge Foundation"
	@echo "All rights reserved"
	@echo ""
	@echo "$(COLOR_WHITE)Usage:$(COLOR_RESET)"
	@echo "  make $(COLOR_BLUE)COMMAND$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_WHITE)Commands:$(COLOR_RESET)"
	@echo "  $(COLOR_BLUE)setup$(COLOR_RESET)        - Setup testbed environment (i.e. build Docker images, ...) - usually executed once only"
	@echo "  $(COLOR_BLUE)clean$(COLOR_RESET)        - Clean up testbed environment (i.e. remove Docker image and container, ...)"
	@echo "  $(COLOR_BLUE)start$(COLOR_RESET)        - Start testbed environment (see 'docker-compose-testbed.yml')"
	@echo "  $(COLOR_BLUE)stop$(COLOR_RESET)         - Stop testbed environment (see 'docker-compose-testbed.yml')"
	@echo "  $(COLOR_BLUE)status$(COLOR_RESET)       - State of the running testbed environment"
	@echo "  $(COLOR_BLUE)sandbox$(COLOR_RESET)      - Build a (Rust) sandbox for compiling Centrifuge chain in a containerized environment (optional)"
	@echo ""
endef

# Initialize testbed environment
#
# This function create a Docker image for a sandbox (i.e. a playground inside which
# the chain executable is compiled)
define setup
	$(call _build_chain_docker_image)
endef

# Clean up testbed components (i.e. Docker image, containers, ...)
define clean
	$(call _stop_sidecar_and_chain_containers)
	$(call _remove_sandbox_docker_image)
	$(call _remove_chain_docker_image)
	$(call _purge_docker_images_and_containers)
endef

# Start complete testbed environment
define start
	$(_start_sidecar_and_chain_containers)
endef 

# Stop complete testbed environment
define stop
	$(_stop_sidecar_and_chain_containers)
endef 

# Show actual running status of the testbed environment
define status
	$(_show_testbed_environment_state)
endef 

# Build a Docker image for an optional containerized Rust sandbox
define sandbox
	$(_build_sandbox_docker_image)
endef


# ------------------------------------------------------------------------------
# PRIVATE FUNCTIONS
# ------------------------------------------------------------------------------

# Build compilation sandbox (first stage of the Dockerfile)
#
# This is a convenient function if you wanna build the chain manually, inside
# a Docker container, using the following command:
#
# $ docker container run --rm -it --volume `pwd`/..
define _build_sandbox_docker_image
	@echo "Build sandbox Docker image $(COLOR_BLUE)$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image build \
		--build-arg ARG_PROFILE="release" \
		--build-arg ARG_RUST_TOOLCHAIN=$(RUST_TOOLCHAIN) \
		--no-cache \
		--tag=$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		--rm \
		--file=./docker/Dockerfile \
		--target=sandbox \
		.
endef

# Build the Centrifuge chain Docker image (used in 'docker/docker-compose.yml' file)
define _build_chain_docker_image
	@echo "Build Centrifuge chain's Docker image $(COLOR_BLUE)$(CHAIN_DOCKER_IMAGE_NAME):$(CHAIN_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image build \
		--build-arg ARG_PROFILE="release" \
		--build-arg ARG_CHAIN_VERSION=$(CHAIN_VERSION) \
		--build-arg ARG_IMAGE_BUILD_DATE=$(date) \
		--no-cache \
		--tag=$(CHAIN_DOCKER_IMAGE_NAME):$(CHAIN_DOCKER_IMAGE_TAG) \
		--rm \
		--file=./docker/Dockerfile \
		--target=service \
		.
endef

# Remove compilation sandbox from local Docker repository
define _remove_sandbox_docker_image
	@echo "Remove (local) sandbox Docker image $(COLOR_BLUE)$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image rm $(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) > /dev/null 2>&1 || true
endef

# Remove chain's Docker image from local Docker repository
define _remove_chain_docker_image
	@echo "Remove (local) chain Docker image $(COLOR_BLUE)$(CHAIN_DOCKER_IMAGE_NAME):$(CHAIN_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image rm $(CHAIN_DOCKER_IMAGE_NAME):$(CHAIN_DOCKER_IMAGE_TAG) > /dev/null 2>&1 || true
endef

# Remove dangling Docker images and stopped containers
#
# When building a Docker image, it is frequent that this process
# crashes, producing intermediate (hence dangling) containers and
# file system layers. This function helps cleaning up such 'junks'.
# This function should be called at the end of an image building
# process.
define _purge_docker_images_and_containers
	@docker ps -aq --no-trunc | xargs docker rm
	@docker images -q --filter dangling=true | xargs docker rmi
endef

# Start complete testbed components
define _start_sidecar_and_chain_containers
	@docker-compose \
		-f ./docker/docker-compose-networks.yml \
		-f ./docker/docker-compose-services.yml \
		up --detach \
		--force-recreate
endef

# Stop tesbed environment and remove eventual orphans containers
define _stop_sidecar_and_chain_containers
	@docker-compose \
		-f ./docker/docker-compose-networks.yml \
		-f ./docker/docker-compose-services.yml \
		down --remove-orphans
endef

# Display current running status of the testbed environment
define _show_testbed_environment_state
	@docker-compose \
		-f ./docker/docker-compose-networks.yml \
		-f ./docker/docker-compose-services.yml \
		ps
	@docker network inspect $(DOCKER_NETWORK_NAME)
endef


# ------------------------------------------------------------------------------
# TARGETS DEFINITION
# ------------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help setup clean start stop status sandbox

# Set default target if none is specified
.DEFAULT_GOAL := help

help:
	$(call help)

setup:
	$(call setup)

clean:
	$(call clean)

start:
	$(call start)

stop:
	$(call stop)

status:
	$(call status)

sandbox:
	$(call sandbox)