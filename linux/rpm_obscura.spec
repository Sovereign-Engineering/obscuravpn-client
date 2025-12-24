Name:           obscura
Version:        0.0.1
Release:        1
Summary:        Obscura VPN client
License:        PolyForm-Noncommercial-1.0.0
URL:            https://obscura.net

%description
Privacy that's more than a promise.

%install
install -Dm755 %{_sourcedir}/obscura %{buildroot}%{_bindir}/obscura
install -Dm644 %{_sourcedir}/obscura.service %{buildroot}%{_unitdir}/obscura.service
install -Dm644 %{_sourcedir}/obscura-sysusers.conf %{buildroot}%{_sysusersdir}/obscura.conf
install -Dm644 %{_sourcedir}/obscura-preset.conf %{buildroot}%{_presetdir}/80-obscura.preset

%files
%{_bindir}/obscura
%{_unitdir}/obscura.service
%{_sysusersdir}/obscura.conf
%{_presetdir}/80-obscura.preset

%pre
%sysusers_create_package obscura %{_sourcedir}/obscura-sysusers.conf

%post
%systemd_post obscura.service
if [ $1 -eq 1 ]; then
    systemctl start obscura.service
fi

%preun
%systemd_preun obscura.service

%postun
%systemd_postun_with_restart obscura.service

%changelog
* Thu Jan 01 1970 obscura authors <support@obscura.net> - 0.0.1-1
- Release
