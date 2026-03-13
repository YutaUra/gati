{
  lib,
  rustPlatform,
  fetchFromGitHub,
  pkg-config,
  cmake,
  oniguruma,
  nix-update-script,
}:

rustPlatform.buildRustPackage {
  pname = "gati";
  version = "0.7.0";

  src = fetchFromGitHub {
    owner = "YutaUra";
    repo = "gati";
    rev = "v${version}";
    hash = "";
  };

  cargoHash = "sha256-XFIbveWGN38n3lna4ZrMB0z8qtnNLgeJixCYaZcYV68=";

  nativeBuildInputs = [ pkg-config cmake ];

  buildInputs = [ oniguruma ];

  RUSTONIG_SYSTEM_LIBONIG = 1;

  # cli_clipboard requires a display server, unavailable in the sandbox
  checkFlags = [ "--skip=app::tests::export_sets_flash_message_on_success" ];

  passthru.updateScript = nix-update-script { };

  meta = with lib; {
    description = "A terminal tool for reviewing code, not writing it";
    homepage = "https://github.com/YutaUra/gati";
    license = licenses.mit;
    maintainers = with maintainers; [ yutaura ];
    mainProgram = "gati";
    platforms = platforms.unix;
  };
}
