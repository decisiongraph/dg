# dg + dg-mcp from the latest GitHub release binaries. Takes any pkgs, so
# consumers can callPackage it with their own nixpkgs: evaluates in
# milliseconds and never compiles. release.json is written by release.yml.
{ lib, stdenv, fetchurl, autoPatchelfHook, zlib }:

let
  release = lib.importJSON ./release.json;
  system = stdenv.hostPlatform.system;
  asset = release.assets.${system}
    or (throw "dg ${release.version} has no release binary for ${system}");
  # musl builds are fully static; glibc ones (before 0.1.18) need patching
  isGnu = lib.hasSuffix "-linux-gnu" asset.target;
in
stdenv.mkDerivation {
  pname = "dg";
  inherit (release) version;
  src = fetchurl {
    url = "https://github.com/decisiongraph/dg/releases/download/v${release.version}/dg-${asset.target}.tar.gz";
    inherit (asset) hash;
  };
  sourceRoot = asset.target;
  nativeBuildInputs = lib.optionals isGnu [ autoPatchelfHook ];
  buildInputs = lib.optionals isGnu [ stdenv.cc.cc.lib zlib ];
  dontConfigure = true;
  dontBuild = true;
  dontStrip = true;
  installPhase = ''
    runHook preInstall
    install -Dm755 dg dg-mcp -t $out/bin
    runHook postInstall
  '';
  doInstallCheck = true;
  installCheckPhase = ''
    $out/bin/dg --version
  '';
  meta = {
    description = "Decision Graph — markdown-as-database CLI and MCP server (prebuilt)";
    homepage = "https://github.com/decisiongraph/dg";
    mainProgram = "dg";
    sourceProvenance = [ lib.sourceTypes.binaryNativeCode ];
    platforms = builtins.attrNames release.assets;
  };
}
