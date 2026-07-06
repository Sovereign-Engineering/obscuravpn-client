%global __brp_strip %{nil}

Name:           obscura-cli
Version:        @VERSION@
Release:        1
Summary:        Obscura VPN command-line client and service
License:        PolyForm-Noncommercial-1.0.0
URL:            https://obscura.com
Packager:       Obscura Repository Signer <packages@obscura.com>
Requires:       shadow-utils

%description
Privacy that's more than a promise.

%package -n obscura-gui
Summary:        Obscura VPN desktop application
Requires:       obscura-cli = %{version}-%{release}
%description -n obscura-gui
Privacy that's more than a promise.

%package -n obscura
Summary:        Obscura VPN
BuildArch:      noarch
Requires:       obscura-gui
%description -n obscura
Privacy that's more than a promise.

%package -n obscura-repository
Summary:        Obscura VPN dnf repository configuration and signing key
BuildArch:      noarch
%description -n obscura-repository
Privacy that's more than a promise.

%install
install -Dm755 %{_sourcedir}/obscura %{buildroot}%{_bindir}/obscura
install -Dm644 /repo/linux/common/obscura.service %{buildroot}%{_unitdir}/obscura.service
install -Dm644 /repo/linux/common/obscura-sysusers.conf %{buildroot}%{_sysusersdir}/obscura.conf
install -Dm644 /repo/linux/common/obscura-preset.conf %{buildroot}%{_presetdir}/80-obscura.preset
install -Dm755 %{_sourcedir}/obscura-gui %{buildroot}%{_bindir}/obscura-gui
install -Dm644 /repo/linux/common/net.obscura.vpn.gui.desktop %{buildroot}%{_datadir}/applications/net.obscura.vpn.gui.desktop
install -Dm644 /repo/linux/common/icons/net.obscura.vpn.gui-128.png %{buildroot}%{_datadir}/icons/hicolor/128x128/apps/net.obscura.vpn.gui.png
install -Dm644 /repo/linux/common/icons/net.obscura.vpn.gui-256.png %{buildroot}%{_datadir}/icons/hicolor/256x256/apps/net.obscura.vpn.gui.png
install -Dm644 %{_sourcedir}/RPM-GPG-KEY-obscura %{buildroot}%{_sysconfdir}/pki/rpm-gpg/RPM-GPG-KEY-obscura
install -Dm644 %{_sourcedir}/RPM-GPG-KEY-obscura-revoked %{buildroot}%{_sysconfdir}/pki/rpm-gpg/RPM-GPG-KEY-obscura-revoked
install -Dm644 %{_sourcedir}/obscura.repo %{buildroot}%{_sysconfdir}/yum.repos.d/obscura.repo
install -Dm644 /repo/LICENSE.md %{buildroot}%{_defaultlicensedir}/obscura-cli/LICENSE.md
install -Dm644 /repo/LICENSE.md %{buildroot}%{_defaultlicensedir}/obscura-gui/LICENSE.md
install -Dm644 /repo/LICENSE.md %{buildroot}%{_defaultlicensedir}/obscura/LICENSE.md
install -Dm644 /repo/LICENSE.md %{buildroot}%{_defaultlicensedir}/obscura-repository/LICENSE.md
install -Dm755 /repo/linux/rpm/obscura-package-signing-key-refresh.bash %{buildroot}%{_libexecdir}/obscura-package-signing-key-refresh
install -Dm644 /repo/linux/rpm/obscura-package-signing-key-refresh.service %{buildroot}%{_unitdir}/obscura-package-signing-key-refresh.service
install -Dm644 /repo/linux/rpm/obscura-package-signing-key-refresh.timer %{buildroot}%{_unitdir}/obscura-package-signing-key-refresh.timer
install -Dm644 /repo/linux/rpm/obscura-repository-preset.conf %{buildroot}%{_presetdir}/80-obscura-repository.preset

%files -n obscura-cli
%license %{_defaultlicensedir}/obscura-cli/LICENSE.md
%{_bindir}/obscura
%{_unitdir}/obscura.service
%{_sysusersdir}/obscura.conf
%{_presetdir}/80-obscura.preset

%files -n obscura-gui
%license %{_defaultlicensedir}/obscura-gui/LICENSE.md
%{_bindir}/obscura-gui
%{_datadir}/applications/net.obscura.vpn.gui.desktop
%{_datadir}/icons/hicolor/128x128/apps/net.obscura.vpn.gui.png
%{_datadir}/icons/hicolor/256x256/apps/net.obscura.vpn.gui.png

%files -n obscura
%license %{_defaultlicensedir}/obscura/LICENSE.md

%files -n obscura-repository
%license %{_defaultlicensedir}/obscura-repository/LICENSE.md
%{_sysconfdir}/pki/rpm-gpg/RPM-GPG-KEY-obscura
%{_sysconfdir}/pki/rpm-gpg/RPM-GPG-KEY-obscura-revoked
%config(noreplace) %{_sysconfdir}/yum.repos.d/obscura.repo
%{_libexecdir}/obscura-package-signing-key-refresh
%{_unitdir}/obscura-package-signing-key-refresh.service
%{_unitdir}/obscura-package-signing-key-refresh.timer
%{_presetdir}/80-obscura-repository.preset

%pre -n obscura-cli
%sysusers_create_package obscura /repo/linux/common/obscura-sysusers.conf

%post -n obscura-cli
%systemd_post obscura.service
if [ $1 -eq 1 ]; then
    systemctl start obscura.service || true
fi

%preun -n obscura-cli
%systemd_preun obscura.service

%postun -n obscura-cli
%systemd_postun_with_restart obscura.service

%post -n obscura-repository
%systemd_post obscura-package-signing-key-refresh.timer
if [ $1 -eq 1 ]; then
    systemctl start obscura-package-signing-key-refresh.timer || true
fi

%preun -n obscura-repository
%systemd_preun obscura-package-signing-key-refresh.timer

%postun -n obscura-repository
%systemd_postun_with_restart obscura-package-signing-key-refresh.timer

%changelog
* @DATE@ Obscura Repository Signer <packages@obscura.com> - @VERSION@-1
- Release
