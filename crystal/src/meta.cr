require "./version"
require "./meta/doctor"

module Meta
  # Dispatch meta subcommand.
  # Returns true if handled, false if not a meta command.
  def self.dispatch(args : Array(String)) : Bool
    return false if args.empty?
    return false unless args[0] == "meta"

    subcommand = args[1]?

    case subcommand
    when "help", nil
      help
    when "doctor"
      doctor
    else
      STDERR.puts "Unknown meta subcommand: #{subcommand}"
      STDERR.puts "Available: help, doctor"
      exit 1
    end

    true
  end

  def self.help(io : IO = STDOUT)
    git_version = Doctor.git_info.version || "unknown"
    io.puts <<-HELP
git-all v#{GitAll::VERSION} (git #{git_version}) - parallel git across many repositories

USAGE:
    git-all [OPTIONS] <COMMAND> [ARGS...]

OPTIONS:
    -n, --workers <NUM>   Number of parallel workers (default: 8, 0=unlimited)
    --dry-run             Print exact commands without executing
    --ssh                 Use SSH URLs
    --https               Use HTTPS URLs
    -h, --help            Print help information
    -V, --version         Print version

COMMANDS:
    status    Git status with condensed output
    pull      Git pull with condensed output
    fetch     Git fetch with condensed output
    meta      git-all internal commands (help, doctor)
    <any>     Pass-through to git verbatim

META SUBCOMMANDS:
    git-all meta help     Show this help message
    git-all meta doctor   Diagnose git-all and git environment

EXAMPLES:
    git-all pull                  Pull all repos
    git-all status                Status of all repos
    git-all --dry-run pull        Show commands without executing
    git-all -n 4 fetch            Fetch with 4 workers
    git-all checkout main         Switch all repos to main
    git-all meta doctor           Check environment
HELP
  end

  def self.doctor
    puts "git-all doctor"
    puts "=" * 50
    puts

    # git-all info
    puts "GIT-ALL"
    fit = Doctor.fit_info
    puts "  Version:        #{fit.version}"
    puts "  Path:           #{fit.path}"
    puts "  Implementation: Crystal"
    puts

    # Git info
    puts "GIT"
    git = Doctor.git_info
    if git.installed
      puts "  Version:        #{git.version}"
      puts "  Path:           #{git.path}"
      default_branch = Doctor.git_default_branch
      puts "  Default branch: #{default_branch || "(not configured)"}"
    else
      puts "  Status:         NOT FOUND"
    end
    puts

    # Git add-ons
    puts "GIT ADD-ONS"
    addons = Doctor.git_addons
    installed = addons.select(&.installed)
    not_installed = addons.reject(&.installed)

    if installed.empty? && not_installed.empty?
      puts "  (none checked)"
    else
      installed.each do |addon|
        version_info = addon.version ? " (#{addon.version})" : ""
        puts "  [x] #{addon.name}#{version_info}"
      end
      not_installed.each do |addon|
        puts "  [ ] #{addon.name}"
      end
    end
    puts

    # Workstation info
    puts "WORKSTATION"
    puts "  OS:        #{Doctor.os_name} #{Doctor.os_version}"
    puts "  Shell:     #{Doctor.shell}"
    puts "  CPU cores: #{Doctor.cpu_count}"
    puts "  Workers:   #{DEFAULT_WORKERS} (git-all default)"
    puts
  end
end
