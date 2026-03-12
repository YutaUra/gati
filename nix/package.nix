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
  version = "0.5.0";

  src = fetchFromGitHub {
    owner = "YutaUra";
    repo = "gati";
    rev = "v${version}";
    hash = "sha256-0zuu3bewFQ2ArlLxBRS6JMJPCyqZCOd/R+vCo9ZahvA=";
  };

  cargoHash = "sha256-zqBSkEcNp21x8Ub0WtVlXLiFsTGlu5PFb3J6oLR54R4=";

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
