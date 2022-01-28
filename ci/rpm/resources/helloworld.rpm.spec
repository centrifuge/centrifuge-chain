Name:           helloworld
Version:        1
Release:        0
Summary:        Sample hello world Rust program as an example

Group:          TecAdmin
BuildArch:      noarch
License:        GPL
URL:            https://github.com/tecrahul/mydumpadmin.git
Source0:        helloworld-0.0.1.tgz

%description
Write some description about your package here

%prep
# Expand source archive
tar -x 
%setup -q
%build
cargo build --release
%install
install target/release/helloworld /sbin
install -m 0755 /sbin/helloworld

%files
/bin/helloworld

%changelog
* Tue Oct 24 2017 Rahul Kumar  1.0.0
  - Initial rpm release