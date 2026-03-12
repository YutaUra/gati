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
  version = "0.1.0";

  src = fetchFromGitHub {
    owner = "YutaUra";
    repo = "gati";
    rev = "v${version}";
    hash = "sha256-LP6FL/PnuCnl9YsSh21cLRJJ/dEXYzIzWQVTdThTL/U=";
  };

  cargoHash = "sha256-8feet/TNUvvGOfn5W/Z7Q/fbKzB49HKH7nEG4CARpjM=";

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
