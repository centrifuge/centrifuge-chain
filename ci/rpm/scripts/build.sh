
#!/bin/bash

# -------------------------------------------------------------------------------------------------
# Variables definition
# -------------------------------------------------------------------------------------------------

# Terminal colors definition
RED='\E[1;31m'
YELLOW='\E[1;33m'
GREEN='\E[1;32m'
BLUE='\E[1;34m'
RESET='\E[0m'

# TODO: for testing only
PACKAGE_VERSION=0.0.1

# Special characters
CHECK_MARK_CHAR='\xE2\x9C\x94'

# Command line arguments definition
CMDLINE_OPTIONS_SHORT=a:g:ht:s:
CMDLINE_OPTIONS_LONG=help,git:,tar:,arch:,spec:

# RPM packaging folder
RPMBUILD_FOLDER="$HOME/rpmbuild"

# -------------------------------------------------------------------------------------------------
# Functions implementation
# -------------------------------------------------------------------------------------------------

# Display command's help message
function show_help {
    echo ""
    echo "USAGE: build OPTIONS"
    echo ""
    echo "OPTIONS:"
    echo "  --tar | -t      Source code passed as a compressed archive (e.g. helloworld-0.0.1.tgz)"
    echo "  --git | -g      Source code pulled from the give git repository URL (e.g. centrifuge/centrifuge-chain@master)"
    echo "  --arch | -a     CPU architecture for which the binary RPM must be built (e.g. x86_64,...)"
    echo "  --spec | -s     RPM package specification file"
    echo ""
    echo "  Options --tar and --git are mutually exclusive. Only one of them can be specified!"
    echo ""
    echo "EXAMPLES:"
    echo "  To build an RPM package from a given source archive (or tarball), proceed as follows:"
    echo "    $ build --tar helloworld-0.0.1.tgz --spec helloworld.spec"
    echo ""
    echo "  To build an RPM package from a Git repository on Github, from master branch, here's how to proceed:"
    echo "    $ build --git https://github.com/hello/helloworld.git@master --spec helloworld.spec"
}

# Check if command-line options are used properly
#
# This function essentially checks if '--tar' and '--git' are not both given, as they are mutually exclusive. Only one
# can be used but not both at the same time.
function check_options {
    # Only one of tar or git source otpions can be used at once, not both
    if [ -v OPTION_SOURCE_TAR ] && [ -v OPTION_SOURCE_GIT ]; then
        echo -e "${RED}[ERROR]${RESET} Options ${BLUE}'--tar'${RESET} and ${BLUE}'--git'${RESET} are mutually exclusive and cannot be used at the same time!" >&2
        exit 3
    fi

    # Be sure a spec file is available
    if [ ! -v OPTION_SPECFILE ]; then
        echo -e "${RED}[ERROR]${RESET} No RPM spec file is given (see ${BLUE}'--spec'${RESET} option)" >&2
        exit 4
    fi
}

# Setup RPM packaging environment
function setup_rpm_environment {
    if [[ ! -d "$RPMBUILD_FOLDER" ]]; then
        echo -e "    ${GREEN}$CHECK_MARK_CHAR${RESET} setting up RPM packaging environment in ${BLUE}$RPMBUILD_FOLDER${RESET}"
        mkdir -p /home/rpmbuilder/rpmbuild/{BUILD,SPECS,SOURCES,BUILDROOT,RPMS,SRPMS,tmp}
        chmod -R 777 /home/rpmbuilder/rpmbuild
    fi
}

# Install files to their proper location in RPM packaging environment
function install_rpm_files {
    if [ -v OPTION_SOURCE_TAR ]; then
        cp $OPTION_SOURCE_TAR $RPMBUILD_FOLDER/SOURCES
        echo -e "    ${GREEN}$CHECK_MARK_CHAR${RESET} installing source code's tar archive ${BLUE}$OPTION_SOURCE_TAR${RESET} in SOURCES folder"
    else
        echo -e "    ${GREEN}$CHECK_MARK_CHAR${RESET} pulling source code from Git repository ${BLUE}$OPTION_SOURCE_GIT${RESET} and installing it in SOURCES folder"
        
        # Clone source code repository and create source tar archive
        git clone $OPTION_SOURCE_GIT "$RPMBUILD_FOLDER/tmp/$(basename ${OPTION_SOURCE_GIT%.*})"
        tar -czf "$RPMBUILD_FOLDER/SOURCES/$(basename ${OPTION_SOURCE_GIT%.*})-$PACKAGE_VERSION.tar.gz" -C "$RPMBUILD_FOLDER/tmp" "$(basename ${OPTION_SOURCE_GIT%.*})"
    fi

    # Install RPM spec file
    cp $OPTION_SPECFILE $RPMBUILD_FOLDER/SPECS
    echo -e "    ${GREEN}$CHECK_MARK_CHAR${RESET} installing RPM spec file ${BLUE}$OPTION_SPECFILE${RESET} in SPECS folder"
}

# Build RPM source and binary packages
function build_rpm_packages {
    echo ""
    echo -e "    ${GREEN}$CHECK_MARK_CHAR${RESET} building source and binary RPM packages... be patient :)"
}

# -------------------------------------------------------------------------------------------------
# Script entry point
# -------------------------------------------------------------------------------------------------

# Test if getopt is an enhanced version
getopt --test > /dev/null
if [[ $? -ne 4 ]]; then
    echo -e "${RED}[ERROR]${RESET} The ${BLUE}`getopt --test`${RESET} command failed in this environment."
    exit 1
fi

# Parse command-line options
OPTIONS=$(getopt --options $CMDLINE_OPTIONS_SHORT --longoptions $CMDLINE_OPTIONS_LONG --name "$0" -- "$@")
if [[ $? -ne 0 ]]; then
    echo -e "${RED}[ERROR]${RESET} Cannot parse command-line options... exiting." >&2
    exit 2
fi

# Use eval with "$OPTIONS" to properly handle the quoting
eval set -- "$OPTIONS"

# Process options up to the argument mark (i.e. '--')
while true; do
    case "$1" in
        -h|--help)
            show_help
            exit 1
            ;;
        -t|--tar)
            OPTION_SOURCE_TAR=$2
            shift 2             
            ;;
        -g|--git)
            OPTION_SOURCE_GIT=$2
            shift 2         
            ;;
        -s|--spec)
            OPTION_SPECFILE=$2
            shift 2         
            ;;
        -- )
            # skip over separation between options and arguments
            shift
            break
            ;;
        *)
            echo -e "${RED}Internal error!${RESET}"
            exit 3
            ;;
    esac
done

echo ""

# Be sure command-line options are used properly
check_options

# Setup RPM build environment (if necessary)
setup_rpm_environment

# Install package files in RPM packaging environment
install_rpm_files

# Build source and binary RPM packages
build_rpm_packages

exec bash





