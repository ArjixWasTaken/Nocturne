project('nocturne',
          version: '2.0.0',
    meson_version: '>= 1.0.0',
  default_options: [ 'warning_level=2', 'werror=false', ],
)

i18n = import('i18n')
gnome = import('gnome')

cargo = find_program('cargo')
cargo_target_dir = meson.project_build_root() / 'cargo-target'
profile = get_option('buildtype') == 'debug' ? 'debug' : 'release'

custom_target('nocturne',
  build_by_default: true,
  build_always_stale: true,
  output: 'nocturne',
  console: true,
  install: true,
  install_dir: get_option('bindir'),
  command: [
    'env', 'CARGO_TARGET_DIR=' + cargo_target_dir,
    'NOCTURNE_LOCALEDIR=' + (get_option('prefix') / get_option('localedir')),
    cargo, 'build',
    '--manifest-path', meson.project_source_root() / 'Cargo.toml',
    profile == 'release' ? '--release' : [],
    '&&', 'cp', cargo_target_dir / profile / 'nocturne', '@OUTPUT@',
  ],
)

subdir('data')
subdir('po')

gnome.post_install(
     glib_compile_schemas: true,
    gtk_update_icon_cache: true,
  update_desktop_database: true,
)
