###################################################################################################
# Centrifuge Chain                                                                                #
# Infrastructure-as-Code                                                                          #
#                                                                                                 #
# Makefile                                                                                        #
#                                                                                                 #
# Handcrafted since 2022 by Centrifuge contributors.                                              #
# All rights reserved                                                                             #
#                                                                                                 #
#                                                                                                 #
# Description: Main script used to build RPM builder Docker images for major RPM-based Linux      #
#              distributions, including CentOS, Red Hat, and Fedora (and Scientific Linux).       #
###################################################################################################


# -------------------------------------------------------------------------------------------------
# VARIABLES DEFINITION
# -------------------------------------------------------------------------------------------------

# Colors definition
RED=\033[0;31m
RESET=\033[38;5;112m
BLUE=\033[38;5;33m
YELLOW=\033[0;33m
WHITE=\033[97m
RESET=\033[0m

# Docker image name and tag
DOCKER_IMAGE:="centrifugeio/rpmbuilder"
DOCKER_IMAGE_TAG:="latest"
DOCKER_FILE:="Dockerfile.rpmbuilder"

# Supported Linus distros
SUPPORTED_LINUX_DISTROS:=centos fedora redhat

# -------------------------------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# -------------------------------------------------------------------------------------------------

# Display help/usage message
define display_help_message
	@echo ""
	@echo "Centrifuge Chain"
	@echo "Infrastructure-as-Code"
	@echo ""
	@echo "Handcrafted since 2022 by Centrifuge contributors"
	@echo "All rights reserved"
	@echo ""
	@echo "$(WHITE)USAGE:$(RESET)"
	@echo "  make $(BLUE)COMMAND$(RESET) [$(YELLOW)OPTIONS$(RESET)]"
	@echo ""
	@echo "$(WHITE)COMMANDS:$(RESET)"
	@echo "  $(BLUE)build-image$(RESET)  [$(YELLOW)centos$(RESET) | $(YELLOW)fedora$(RESET) | $(YELLOW)redhat$(RESET)]     build RPM builder's Docker image for a given Linux distro"
	@echo "  $(BLUE)remove-image$(RESET) [$(YELLOW)centos$(RESET) | $(YELLOW)fedora$(RESET) | $(YELLOW)redhat$(RESET)]     remove RPM builder's Docker image of a given Linux distro from local repository"
	@echo "  $(BLUE)clean$(RESET)                                       remove builds and logs"
	@echo "  $(BLUE)mrproper$(RESET)                                    remove builds, logs, and Docker images (including dangling images and stopped containers)"
	@echo ""
	@echo "$(WHITE)EXAMPLES:$(RESET)"
	@echo "  Create a RPM builder's Docker image for Red Hat Linux:"
	@echo "    $(BLUE)$$ make build-image redhat$(RESET)"
	@echo "  This command will produce a new Docker image called 'centrifugeio/rpmbuilder:redhat-latest'"
	@echo ""
	@echo "  For removing a RPM builders' Docker image for a given Linux distro, enter the following command:"
	@echo "    $(BLUE)$$ make remove-image fedora$(RESET)"
	@echo "  This command will remove the Docker image called 'centrifugeio/rpmbuilder:fedora-latest'"
endef

# Build a RPM builder Docker image for a given RPM-based Linux distro.
#
# During the process of building an image Docker will step through the instructions in your Dockerfile
# executing each in the order specified. As each instruction is examined Docker will look for an existing 
# image in its cache that it can reuse, rather than creating a new (duplicate) image. If you do not want 
# to use the cache at all you can use the --no-cache=true option on the docker build command. That's what
# we do in this function, rebuilding a clean image.
#
# @param $1 Name of the Linux distro for which a RPM builer Docker image has to be built. Supported
#           Linux distros are 'centos', 'fedora', 'redhat'.
# @see https://docs.docker.com/engine/userguide/eng-image/dockerfile_best-practices/#build-cache
define build_docker_image
	@echo "$(RESET)[INFO]$(RESET) Building $(BLUE)$(1)$(RESET) RPM builder image ..."
	@docker image build \
		--target rpmbuilder-$(1) \
		--tag="$(DOCKER_IMAGE):$(1)-$(DOCKER_IMAGE_TAG)" \
		--no-cache \
		--rm \
		--file=$(DOCKER_FILE) \
		.
endef

# Remove the RPM builder image of a given Linux distro from local Docker repository
#
# @param $1 Name of the Linux distro for which a RPM builer Docker image is to be built. Available
#           values are 'centos', 'fedora', 'redhat'.
define remove_docker_image
	@echo "$(RED)[WARN] $(ccend)$(WHITE) Removing $(BLUE)$(1)$(RESET) RPM builder image ..."
	@docker image rm --force $(DOCKER_IMAGE):$(1)-$(DOCKER_IMAGE_TAG) > /dev/null 2>&1 || true 
endef

# Clean up RPM building environment (in ./rpmbuild folder)
define clean_builds_and_logs
	@rm -rf ./rpmbuild .bash_history .ssh
endef

# Remove dangling Docker images and stopped containers
define prune_docker_images_and_containers
	@docker ps -aq --no-trunc | xargs docker rm
	@docker images -q --filter dangling=true | xargs docker rmi
endef

# -------------------------------------------------------------------------------------------------
# TARGETS DEFINITION
# -------------------------------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help build-image remove-image clean mrproper

# Turn arguments for 'build-image' and 'remove-image' targets into "do-nothing" targets
#
# If the first argument is 'build-image' or 'remove-image', then following argument(s) is(are)
# a list of Linux distribution(s) for which a RPM builder Docker image has to be built. For instance,
# in the following command, 'redhat' and 'centos' are arguments passed to the 'build-image' or
# 'remove-image' target:
#   $ make [build-image | remove-image] redhat centos
# Here, 'redhat' and 'centos' are turned into "do-nothing" targets.
#
# See https://stackoverflow.com/questions/2214575/passing-arguments-to-make-run/14061796#14061796
ifeq ($(firstword $(MAKECMDGOALS)),$(filter $(MAKECMDGOALS), build-image remove-image))
  # use the rest as arguments for "build-image"
  DISTRO_ARGS := $(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS))

  # ...and if okay, turn them into do-nothing targets
  $(eval $(DISTRO_ARGS):;@:)
endif

# Set default target if none is specified
.DEFAULT_GOAL := help

all: help

# Show help message
help:
	$(call display_help_message)

# Build RPM builder's Docker image for a given (or more) Linux distribution
build-image:
	$(foreach distro,$(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS)),$(call build_docker_image,$(distro)))

# Remove one (or all) RPM builder's Docker image(s)
remove-image:
	$(foreach distro,$(wordlist 2,$(words $(MAKECMDGOALS)),$(MAKECMDGOALS)),$(call remove_docker_image,$(distro)))

# Gentle clean up (only builds and logs)
clean:
	@echo "$(RED)[WARN] $(RESET)$(WHITE) Removing builds and logs ..."
	$(call clean_builds_and_logs)

# Remove dangling Docker images and unused stopped containers
mrproper: clean
	@echo "$(RED)[WARN] $(RESET)$(WHITE) Removing builds, logs, and Docker images (including dangling Docker images and stopped containers) ..."
	$(call prune_docker_images_and_containers)