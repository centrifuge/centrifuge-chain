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
# Description: Centrifuge's RPM package builder.                                                  #
# See https://whamcloud.github.io/Online-Help/docs/Contributor_Docs/cd_Building_Rust_RPMs.html    #
###################################################################################################


# -------------------------------------------------------------------------------------------------
# VARIABLES DEFINITION
# -------------------------------------------------------------------------------------------------

# Colors definition
ccred=\033[0;31m
ccgreen=\033[38;5;112m
ccblue=\033[38;5;33m
ccyellow=\033[0;33m
ccorange=\033[38;5;166m
ccwhite=\033[97m
ccpink=\033[35;40m
ccend=\033[0m

# Docker image name and tag
DOCKER_IMAGE:="centrifugeio/rpmbuilder"
DOCKER_IMAGE_TAG:="0.1.0"
DOCKER_FILE:="Dockerfile.rpmbuilder"

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
	@echo "$(ccwhite)Usage:$(ccend)"
	@echo "  make $(ccpink)COMMAND$(ccend)"
	@echo ""
	@echo "Commands:"
	@echo "  $(ccpink)build-image$(ccend)      - build (incrementally) RPM builder's Docker image (from cache)"
	@echo "  $(ccpink)rebuild-image$(ccend)    - rebuild RPM builder's Docker image from scratch (without using the cache)"
	@echo "  $(ccpink)remove-image$(ccend)     - remove RPM builder's Docker image from local repository"
	@echo "  $(ccpink)build-spec$(ccend)       - generate RPM spec for Centrifuge Chain using rust2rpm tool"
	@echo "  $(ccpink)build-rpm$(ccend)        - build Centrifuge Chain's binary RPM package (inside a container)"
	@echo "  $(ccpink)mrproper$(ccend)         - remove local dangling Docker images and stopped containers"
	@echo ""
endef

# Build an incremental (using cache) Docker image
define build_docker_image
	@docker image build \
		--tag=$(DOCKER_IMAGE):$(DOCKER_IMAGE_TAG) \
		--tag=$(DOCKER_IMAGE):latest \
		--rm \
		--file=$(DOCKER_FILE) \
		.
endef

# Build a clean Docker image, without using cache
#
# During the process of building an image Docker will step through the instructions in your Dockerfile
# executing each in the order specified. As each instruction is examined Docker will look for an existing 
# image in its cache that it can reuse, rather than creating a new (duplicate) image. If you do not want 
# to use the cache at all you can use the --no-cache=true option on the docker build command.
#
# @see https://docs.docker.com/engine/userguide/eng-image/dockerfile_best-practices/#build-cache

define rebuild_docker_image
	@docker image build \
		--tag=$(DOCKER_IMAGE) \
		--tag=$(DOCKER_IMAGE_NAME):latest \
		--no-cache \
		--rm \
		--file=$(DOCKER_FILE) \
		.
endef

# Remove RPM builder images (from local Docker repository)
define remove_docker_image
	@docker image rm --force $(DOCKER_IMAGE) > /dev/null 2>&1 || true && docker image rm --force $(DOCKER_IMAGE_NAME):latest > /dev/null 2>&1 || true
endef

# Remove dangling Docker images and stopped containers
define purge_docker_images_and_containers
	@docker ps -aq --no-trunc | xargs docker rm
	@docker images -q --filter dangling=true | xargs docker rmi
endef

# Build Centrifuge Chain's RPM package inside a Docker container
define build_rpm_package
	@docker container run --rm -it --volume $(PWD):/home/rpmbuilder $(DOCKER_IMAGE_NAME):latest rpmbuild
endef

# Build Centrifuge Chain's RPM package specification from Cargo files, using rust2rpm tool
define build_rpm_spec
	@docker container run --rm -it --volume $(PWD):/home/rpmbuilder $(DOCKER_IMAGE_NAME):latest rust2rpm
endef

# -------------------------------------------------------------------------------------------------
# TARGETS DEFINITION
# -------------------------------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help build-image rebuild-image remove-image build-rpm build-spec mrproper

# Set default target if none is specified
.DEFAULT_GOAL := help

all: help

# Show help message
help:
	$(call display_help_message)

# Build Docker image incrementally (using cache)
build-image:
	@echo "$(ccgreen)[INFO]$(ccend) Building RPM builder's Docker image (from cache)..."
	$(call build_docker_image)

# Build a clean Docker image, without using cache
rebuild-image:
	@echo "$(ccgreen)[INFO]$(ccend) Rebuilding RPM builder's Docker image (without using cache)..."
	$(call rebuild_docker_image)

# Remove RPM builder's Docker images, as well as containers
remove-image:
	@echo "$(ccred)[WARN] $(ccend)$(ccwhite)Removing RPM builder's local Docker image..."
	$(call remove_docker_image)

# Build Centrifuge Chain's RPM package
build-rpm:
	@echo "$(ccgreen)[INFO]$(ccend)$(ccwhite)Building Centrifuge Chain's RPM package..."
	$(call build_rpm_package)

# Generate Centrifuge Chain's SPEC file from project's Cargo.tom file (using rust2rpm tool)
build-spec:
	@echo "$(ccgreen)[INFO]$(ccend) $(ccwhite)Generating Centrifuge Chain's RPM SEPC file..."
	$(call build_rpm_spec)

# Remove dangling Docker images and unused stopped containers
mrproper:
	@echo "$(ccred)[WARN] $(ccend)$(ccwhite)Removing dangling Docker images and stopped containers ..."
	$(call purge_docker_images_and_containers)