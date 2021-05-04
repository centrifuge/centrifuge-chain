###############################################################################
# Centrifuge                                                                  #
# Cash on Steroids                                                            #
#                                                                             #
# pallets/crowdloan-claim/Makefile                                            #
#                                                                             #
# Handcrafted since 2020 by Centrifugians                                     #
# All rights reserved                                                         #
#                                                                             #
#                                                                             #
# Description: Script for building, testing and benchmarking the crowdloan    #
#              reward module.                                                 #
###############################################################################


# -----------------------------------------------------------------------------
# VARIABLES DEFINITION
# -----------------------------------------------------------------------------

# Colors definition
#include ./tools/automake/colors.mk

# Project's configuration variables
#include ./tools/automake/settings.mk


# -----------------------------------------------------------------------------
# FUNCTIONS DEFINITION
# -----------------------------------------------------------------------------

# Display help/usage message
define display_help_message
	@echo ""
	@echo "$(COLOR_WHITE)Centrifuge$(COLOR_RESET)"
	@echo "$(COLOR_WHITE)Cash on Steroids$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_BLUE)Crowdclaim Claim Pallet$(COLOR_RESET)"
	@echo ""
	@echo "Handcrafted since 2021 by Centrifugians"
	@echo "All rights reserved"
	@echo ""
	@echo "$(COLOR_WHITE)Usage:$(COLOR_RESET)"
	@echo "  make $(COLOR_BLUE)COMMAND$(COLOR_RESET)"
	@echo ""
	@echo "$(COLOR_WHITE)Commands:$(COLOR_RESET)"
	@echo "  $(COLOR_BLUE)build$(COLOR_RESET)                 - Build pallet's binary"
	@echo "  $(COLOR_BLUE)check$(COLOR_RESET)                 - Check pallet's code"
	@echo "  $(COLOR_BLUE)test$(COLOR_RESET)                  - Run unit tests"
	@echo "  $(COLOR_BLUE)metadata$(COLOR_RESET)              - Export pallet metadata to a JSON file"
	@echo ""
endef

# Build pallet
define build_pallet_code
	@cargo build --release
endef

# Check (i.e. compile without generating binary code) Centrifuge chain's source code
define check_pallet_code
	@SKIP_WASM_BUILD=1 cargo check
endef

# Test (i.e. execute unit tests) Centrifuge chain' source code
define test_pallet_code
	@SKIP_WASM_BUILD=1 cargo test
endef

# Save pallet's metadata to a JSON file
define generate_pallet_metadata
	@SKIP_WASM_BUILD=1 cargo test
endef


# -----------------------------------------------------------------------------
# TARGETS DEFINITION
# -----------------------------------------------------------------------------

# NOTE:
# .PHONY directive defines targets that are not associated with files. Generally
# all targets which do not produce an output file with the same name as the target
# name should be .PHONY. This typically includes 'all', 'help', 'build', 'clean',
# and so on.

.PHONY: all help build check test metadata

# Set default target if none is specified
.DEFAULT_GOAL := help

help:
	$(call display_help_message)

build:
	$(call build_pallet_code)

check:
	$(call check_pallet_code)

test:
	$(call test_pallet_code)

metadata:
	$(call generate_pallet_metadata)
