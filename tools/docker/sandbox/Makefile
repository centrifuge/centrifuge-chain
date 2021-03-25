################################################################################
# Centrifuge                                                                   #
# Cash on Steroids                                                             #
#                                                                              #
# tools/docker/sandbox/Makefile                                                #
#                                                                              #
# Handcrafted since 2020 by Centrifuge Project                                 #
# All rights reserved                                                          #
#                                                                              #
#                                                                              #
# Description: Developer sandbox' Docker image building script.                #
################################################################################


# ------------------------------------------------------------------------------
# VARIABLES DEFINITION
# ------------------------------------------------------------------------------

# Colors definition
include ../../automake/colors.mk

# Project settings (e.g. Rust toolchain version, ...)
include ../../automake/settings.mk


# ------------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# ------------------------------------------------------------------------------

# Display help/usage message
define display_help_message
	@echo ""
	@echo "$(COLOR_WHITE)Centrifuge$(COLOR_RESET)"
	@echo "$(COLOR_WHITE)Cash on Steroids$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_BLUE)Sandbox$(COLOR_RESET)"
	@echo ""
	@echo "Handcrafted since 2020 by Centrifuge tribe"
	@echo "All rights reserved"
	@echo ""
	@echo "$(COLOR_WHITE)Usage:$(COLOR_RESET)"
	@echo "  make $(COLOR_BLUE)COMMAND$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_WHITE)Commands:$(COLOR_RESET)"
	@echo "  $(COLOR_BLUE)setup$(COLOR_RESET)        	- Build sandbox' Docker image"
	@echo "  $(COLOR_BLUE)clean$(COLOR_RESET)         - Delete (local) sandbox' Docker image"
	@echo ""
endef


# Build the development sandbox Docker image, with all requisite Rust tools
define build_sandbox_docker_image
	@echo "Build sandbox Docker image $(COLOR_BLUE)$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image build \
		--build-arg ARG_RUST_TOOLCHAIN=$(RUST_TOOLCHAIN) \
		--build-arg ARG_RUST_VERSION=$(RUST_VERSION) \
		--build-arg ARG_IMAGE_BUILD_DATE=$(date) \
		--no-cache \
		--tag=$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		--rm \
		--file="Dockerfile" \
		.
endef

# Remove sandbox Docker image from local repository
define remove_sandbox_docker_image
	@echo "Remove (local) sandbox Docker image $(COLOR_BLUE)$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG)$(COLOR_RESET)"
	@docker image rm $(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) > /dev/null 2>&1 || true
endef


# ------------------------------------------------------------------------------
# TARGETS DEFINITION
# ------------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help setup delete

# Set default target if none is specified
.DEFAULT_GOAL := help

help:
	$(call display_help_message)

setup:
	$(call build_sandbox_docker_image)

clean:
	$(call remove_sandbox_docker_image)
