%global debug_package %{nil}
%global __strip /bin/true
%global _appname OneClient
%global _rpmfile %{_appname}_%{version}_linux_x86_64.rpm

Name:           oneclient-bin
Version:        2.0.1
Release:        1%{?dist}
Summary:        Next-generation open source Minecraft launcher (prebuilt)

License:        GPL-3.0-only
URL:            https://github.com/Polyfrost/OneLauncher
Source0:        %{url}/releases/download/oneclient-%{version}/%{_rpmfile}

ExclusiveArch:  x86_64
BuildRequires:  bsdtar
Requires:       gtk3
Requires:       dbus-libs
Provides:       oneclient = %{version}-%{release}

%description
OneClient is a next-generation open source Minecraft launcher.
This package ships the prebuilt binaries from the upstream release.

%prep
%setup -q -c -T
# Repackages the upstream release rpm; bsdtar reads its cpio payload directly.
bsdtar -xf %{SOURCE0}

%install
mkdir -p %{buildroot}
cp -a usr %{buildroot}/
ln -s oneclient_app %{buildroot}%{_bindir}/oneclient

%files
%{_bindir}/oneclient_app
%{_bindir}/oneclient
%{_datadir}/applications/*.desktop
%{_datadir}/icons/hicolor/*/apps/*

%changelog
* Wed Sep 09 2026 Polyfrost <contact@atmofrost.org> - 2.0.1-1
- See upstream release notes.
