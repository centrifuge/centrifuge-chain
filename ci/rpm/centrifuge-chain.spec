%define __spec_install_post %{nil}
%define __os_install_post %{_dbpath}/brp-compress
%define debug_package %{nil}

Name: centrifuge-chain
Summary: Centrifuge chain implementation in Rust.
Version: @@VERSION@@
Release: @@RELEASE@@%{?dist}
License: LGPL-3.0
Group: Applications/System
Source0: %{name}-%{version}.tar.gz
URL: https://centrifuge.io/

Requires: systemd, shadow-utils
Requires(post): systemd
Requires(preun): systemd
Requires(postun): systemd

BuildRoot: %{_tmppath}/%{name}-%{version}-%{release}-root

%description
%{summary}

%prep
%setup -q

%install
rm -rf %{buildroot}
mkdir -p %{buildroot}
cp -a * %{buildroot}

%post
config_file="/etc/default/centrifuge"
getent group centrifuge >/dev/null || groupadd -r centrifuge
getent passwd centrifuge >/dev/null || \
    useradd -r -g centrifuge -d /home/centrifuge -m -s /sbin/nologin \
    -c "System account for running Centrifuge service" centrifuge
if [ ! -e "$config_file" ]; then
    echo 'CENTRIFUGE_CLI_ARGS=""' > /etc/default/centrifuge
fi
exit 0

%clean
rm -rf %{buildroot}

%files
%defattr(-,root,root,-)
%{_bindir}/*
/usr/lib/systemd/system/centrifuge.service
