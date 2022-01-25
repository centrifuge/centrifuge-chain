#!/bin/bash

# Credits: https://github.com/jc21/docker-rpmbuild-centos7/blob/master/bin/build-spec

RED='\E[1;31m'
YELLOW='\E[1;33m'
GREEN='\E[1;32m'
BLUE='\E[1;34m'
RESET='\E[0m'

getopt --test > /dev/null
if [[ $? -ne 4 ]]; then
    echo -e "${BLUE}I’m sorry, `getopt --test` failed in this environment.${RESET}"
    exit 1
fi


# Enable devtools if variable is set and not empty
if [ -n "$DEVTOOLS" ]; then
    source /opt/rh/devtoolset-7/enable
fi


##############################################
# show_help                                  #
##############################################
function show_help {
    echo "Please specify the name of a spec package to build."
    echo ""
    echo "Usage: $0 [-r dependancy.rpm] /path/to/specfile [/path/to/specfile] [/path/to/specfile]"
    echo "  -m      Disable yum fastest mirror plugin"
    echo "  -u      Uninstalls specified yum package first"
    echo "  -p      Installs specified yum package next"
    echo "  -r      Installs the specified RPM next"
    echo "  -b      Skip broken packages in RPM install, only applies with -r and/or -p"
    echo "  -o      Set yum to ignore obsoletes"
    echo ""
    echo "Example: $0 -r RPMS/x86_64/php.rpm -r RPMS/noarch/slack.rpm SPECS/php-memcache.spec"
    echo ""
}

SHORT=p:u:br:hom
LONG=help

# -temporarily store output to be able to check for errors
# -activate advanced mode getopt quoting e.g. via “--options”
# -pass arguments only via   -- "$@"   to separate them correctly
PARSED=$(getopt --options $SHORT --longoptions $LONG --name "$0" -- "$@")
if [[ $? -ne 0 ]]; then
    # e.g. $? == 1
    #  then getopt has complained about wrong arguments to stdout
    exit 2
fi
# use eval with "$PARSED" to properly handle the quoting
eval set -- "$PARSED"

upackages=
packages=
rpmfiles=
skip_broken=
obsoletes=
nomirror=

# now enjoy the options in order and nicely split until we see --
while true; do
    case "$1" in
        -u)
            upackages+="$2 "
            shift 2
            ;;
        -p)
            packages+="$2 "
            shift 2
            ;;
        -m)
            nomirror="1"
            shift 1
            ;;
        -o)
            obsoletes="--setopt=obsoletes=0"
            shift 1
            ;;
        -r)
            rpmfiles+="$2 "
            shift 2
            ;;
        -h|--help)
            show_help
            exit 1
            ;;
        -b)
            skip_broken="--skip-broken"
            shift
            ;;
        --)
            shift
            break
            ;;
        *)
            echo -e "${RED}Programming error${RESET}"
            exit 3
            ;;
    esac
done

# handle non-option arguments
if [[ $# -eq 0 ]]; then
    show_help
    exit 1
fi

# Yum cleaning
sudo yum clean all
sudo rm -rf /var/cache/yum

cd ~/rpmbuild

## Disable yum mirror if specified
if [ -n "$nomirror" ]; then
    echo -e "${YELLOW}❯ ${GREEN}Disabling Yum Fastest Mirror Plugin ...${RESET}"
    sudo sed -i '/enabled=1/c\enabled=0' /etc/yum/pluginconf.d/fastestmirror.conf
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi
fi

## Uninstall PACKAGES
if [ -n "$upackages" ]; then
    echo -e "${YELLOW}❯ ${GREEN}Uninstalling Yum Packages ...${RESET}"
    sudo yum -y erase $upackages
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi
fi

## Install PACKAGES
if [ -n "$packages" ]; then
    echo -e "${YELLOW}❯ ${GREEN}Installing Yum Packages ...${RESET}"
    sudo yum -y $skip_broken $obsoletes install $packages
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi
fi

## Install RPMS
if [ -n "$rpmfiles" ]; then
    echo -e "${YELLOW}❯ ${GREEN}Installing RPMs ...${RESET}"
    sudo yum -y $skip_broken $obsoletes localinstall $rpmfiles
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi
fi

## Build Spec
echo -e "${YELLOW}❯ ${GREEN}Building ${WHITE}$* ${GREEN}...${RESET}"
cd ~/rpmbuild

for spec in "$@"
do
    sudo yum-builddep -y $skip_broken "${spec}"
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi

    spectool -g -R "${spec}"
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi

    rpmbuild --clean -ba "${spec}"
    rc=$?; if [[ $rc != 0 ]]; then exit $rc; fi
done