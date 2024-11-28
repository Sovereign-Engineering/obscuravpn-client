{ lib, stdenv, fetchFromGitHub, makeWrapper }:

stdenv.mkDerivation rec {
  pname = "create-dmg";
  version = "1.2.2";

  src = fetchFromGitHub {
    owner = "create-dmg";
    repo = pname;
    rev = "v${version}";
    sha256 = "oWrQT9nuFcJRwwXd5q4IqhG7M77aaazBG0+JSHAzPvw=";
  };

  nativeBuildInputs = [ makeWrapper ];

  makeFlags = [ "DESTDIR=${placeholder "out"}" "PREFIX=" ];

  installPhase = ''
    mkdir -p $out/bin
    makeWrapper $src/create-dmg $out/bin/create-dmg
  '';

  meta = with lib; {
    homepage = "https://github.com/create-dmg/create-dmg";
    description = "A shell script to build fancy DMGs";
    license = licenses.mit;
    platforms = with platforms; darwin;
  };
}
