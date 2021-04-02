###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# Makefile                                                                    #
#                                                                             #
# Handcrafted since 2020 by Centrifugians                                     #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Main building script for Centrifuge chain.                     #
###############################################################################


# -----------------------------------------------------------------------------
# VARIABLES DEFINITION
# -----------------------------------------------------------------------------

# Colors definition
include ./tools/automake/colors.mk

# Project's configuration variables
include ./tools/automake/settings.mk


# -----------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# -----------------------------------------------------------------------------

# Display help/usage message
define display_help_message
	@echo ""
	@echo "$(COLOR_WHITE)Centrifuge$(COLOR_RESET)"
	@echo "$(COLOR_WHITE)Cash on Steroids$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_BLUE)Parachain$(COLOR_RESET)"
	@echo ""
	@echo "Handcrafted since 2020 by Centrifugians"
	@echo "All rights reserved"
	@echo ""
	@echo "$(COLOR_WHITE)Usage:$(COLOR_RESET)"
	@echo "  make $(COLOR_BLUE)COMMAND$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_WHITE)Commands:$(COLOR_RESET)"
	@echo "  $(COLOR_BLUE)setup$(COLOR_RESET)                 - Setup project's environment (e.g. developer sandbox, ...)"
	@echo "  $(COLOR_BLUE)clean$(COLOR_RESET)                 - Clean up project (Docker images, binaries, ...)"
	@echo "  $(COLOR_BLUE)build$(COLOR_RESET)                 - Build Centrifuge chain's executable (release)"
	@echo "  $(COLOR_BLUE)check$(COLOR_RESET)                 - Check Centrifuge chain's code (without generating an executable)"
	@echo "  $(COLOR_BLUE)start$(COLOR_RESET)                 - Start a single-node Centrifuge chain (for development)"
	@echo "  $(COLOR_BLUE)test$(COLOR_RESET)                  - Test Centrifuge chain's code"
	@echo "  $(COLOR_BLUE)sandbox-setup$(COLOR_RESET)         - Setup developer sandbox's Docker image"
	@echo "  $(COLOR_BLUE)sandbox-clean$(COLOR_RESET)         - Delete developer sandbox's Docker image"
	@echo ""
endef

# Build developer sandbox Docker image
define setup_sandbox
	@$(MAKE) -C ./tools/docker/sandbox setup
endef

# Clean up project's generated resources
#
# This function remove resources generated while working on this project,
# including binary files, local Cargo index/cache or local Docker images
# of the Centrifuge (developer) sandbox.
define clean_project
	@rm -rf .cargo || true
endef 

# Delete developer sandbox's Docker image
define clean_sandbox
	@$(MAKE) -C ./tools/docker/sandbox clean
endef

# Build Centrifuge chain's executable
define build_chain_executable
	@docker container run \
		--rm -it \
		--memory=$(SANDBOX_CONFIG_MEMORY_SIZE) \
		--cpus=$(SANDBOX_CONFIG_CPUS) \
		--volume $(PWD):/workspace \
		--workdir /workspace \
		$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		cargo build --release	
endef

# Check (i.e. compile without generating binary code) Centrifuge chain's source code
define check_chain_source_code
	@docker container run \
		--rm -it \
		--memory=$(SANDBOX_CONFIG_MEMORY_SIZE) \
		--cpus=$(SANDBOX_CONFIG_CPUS) \
		--volume $(PWD):/workspace \
		--workdir /workspace \
		$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		cargo check --release	
endef

# Test (i.e. execute unit tests) Centrifuge chain' source code
define test_chain_source_code
	@docker container run \
		--rm -it \
		--memory=$(SANDBOX_CONFIG_MEMORY_SIZE) \
		--cpus=$(SANDBOX_CONFIG_CPUS) \
		--volume $(PWD):/workspace \
		--workdir /workspace \
		$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		cargo test --release	
endef

# Start single-node Centrifuge chain for development purpose
define start_single_node_chain
	@docker container run \
		--rm -it \
		--publish 30333:30333 \
		--publish 9933:9933 \
		--publish 9944:9944 \
		--memory=$(SANDBOX_CONFIG_MEMORY_SIZE) \
		--cpus=$(SANDBOX_CONFIG_CPUS) \
		--volume $(PWD):/workspace \
		--workdir /workspace \
		$(SANDBOX_DOCKER_IMAGE_NAME):$(SANDBOX_DOCKER_IMAGE_TAG) \
		target/release/$(CENTRIFUGE_CHAIN_EXECUTABLE) --dev
endef


# -----------------------------------------------------------------------------
# TARGETS DEFINITION
# -----------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help setup clean check test start sandbox-setup sandbox-clean chain-build

# Set default target if none is specified
.DEFAULT_GOAL := help

help:
	$(call display_help_message)

setup: sandbox-setup

clean: sandbox-clean
	$(call clean_project)

build:
	$(call build_chain_executable)

check:
	$(call check_chain_source_code)

test:
	$(call test_chain_source_code)

start:
ifneq ("$(wildcard target/release/$(CENTRIFUGE_CHAIN_EXECUTABLE))", "")
	$(call start_single_node_chain)
else
	@echo "Cannot find Centrifuge chain's executable. Please run $(COLOR_WHITE)make build$(COLOR_RESET) first!"	
endif

sandbox-setup:
	$(call setup_sandbox)

sandbox-clean:
	$(call clean_sandbox)
