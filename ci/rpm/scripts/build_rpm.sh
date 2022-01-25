#!/bin/bash

PACKAGE=mypackage

if [ -z $BUILD_NUMBER ] ; then
 echo BUILD_NUMBER is not know, it is normally given by jenkins
 exit 1
fi

RELEASE=$BUILD_NUMBER
if [ -z $VERSION ] ; then
  echo VERSION is not set via parameter neither via ENV, will use version.txt
  VERSION=`cat version.txt`
fi

cd /home/rpmbuilder
if [ ! -f ./${MYPACKAGE}.spec ] ; then
 echo Sorry, can not find rpm spec file 
 exit 1
fi
cp ${MYPACKAGE}.spec $HOME/rpmbuild/SPECS
# here I patch the spec file to feed it with the version and the release and the date
sed -i -e "s/##VERSION##/${VERSION}/" -e "s/##RELEASE##/${RELEASE}/" /home/rpmbuild/rpmbuild/SPECS/${PACKNAME}.spec
sed -i -e "s/##DATE##/`date +\"%a %b %d %Y\"`/" /home/rpmbuild/rpmbuild/SPECS/${PACKNAME}.spec

# prepare a tar.gz file with the sources and copy it  to the SOURCES directory
...
tar -zcf ${PACKAGE}-${VERSION}-${RELEASE}.tar.gz ./src
cp ${PACKAGE}-${VERSION}-${RELEASE}.tar.gz $HOME/rpmbuild/SOURCES/


# then execute the rpmbuild command
cd $HOME/rpmbuild
rpmbuild -ba --define "_buildnr ${BUILD_NUMBER}" --define "_myversion $VERSION" ./SPECS/${PACKAGE}.spec
# copy the rpms to the artifact directory, for jenkins.
if [[ -d /artifacts ]] ; then
 cp ./RPMS/noarch/${PACKAGE}*.rpm /artifacts/
fi