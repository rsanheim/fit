module Repo
  # Discover git repositories in the current directory (depth 1)
  # Returns sorted array of repo paths for deterministic output
  def self.discover(path : String = Dir.current) : Array(String)
    repos = [] of String

    Dir.each_child(path) do |entry|
      full_path = File.join(path, entry)
      next unless File.directory?(full_path)

      git_path = File.join(full_path, ".git")
      if File.exists?(git_path)
        repos << full_path
      end
    end

    repos.sort!
    repos
  end

  # Extract the repo name from a full path
  def self.name(path : String) : String
    File.basename(path)
  end

  # Format repo name with fixed width for aligned output
  def self.format_name(path : String, width : Int32 = 24) : String
    name = self.name(path)
    if name.size > width
      name[0, width - 3] + "..."
    else
      name.ljust(width)
    end
  end
end
