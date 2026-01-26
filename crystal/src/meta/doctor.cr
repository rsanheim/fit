module Meta::Doctor
  struct ToolInfo
    property name : String
    property version : String?
    property path : String?
    property installed : Bool

    def initialize(@name, @version = nil, @path = nil, @installed = false)
    end
  end

  def self.tool_info : ToolInfo
    path = Process.executable_path || "unknown"
    ToolInfo.new("git-all", GitAll::VERSION, path, true)
  end

  def self.git_info : ToolInfo
    result = run_command("git", ["--version"])
    if result[:success]
      version = result[:stdout].strip.gsub("git version ", "")
      path_result = run_command("which", ["git"])
      path = path_result[:success] ? path_result[:stdout].strip : nil
      ToolInfo.new("git", version, path, true)
    else
      ToolInfo.new("git", installed: false)
    end
  end

  def self.git_default_branch : String?
    result = run_command("git", ["config", "--global", "init.defaultBranch"])
    result[:success] ? result[:stdout].strip : nil
  end

  def self.git_addons : Array(ToolInfo)
    addons = [
      {name: "gh", cmd: "gh", args: ["--version"]},
      {name: "git-lfs", cmd: "git", args: ["lfs", "version"]},
      {name: "delta", cmd: "delta", args: ["--version"]},
      {name: "git-absorb", cmd: "git", args: ["absorb", "--version"]},
      {name: "lazygit", cmd: "lazygit", args: ["--version"]},
    ]

    addons.map do |addon|
      result = run_command(addon[:cmd], addon[:args])
      if result[:success]
        version = result[:stdout].lines.first?.try(&.strip)
        ToolInfo.new(addon[:name], version, installed: true)
      else
        ToolInfo.new(addon[:name], installed: false)
      end
    end
  end

  def self.os_name : String
    result = run_command("uname", ["-s"])
    result[:success] ? result[:stdout].strip : "unknown"
  end

  def self.os_version : String
    result = run_command("uname", ["-r"])
    result[:success] ? result[:stdout].strip : "unknown"
  end

  def self.shell : String
    ENV["SHELL"]? || "unknown"
  end

  def self.cpu_count : Int32
    System.cpu_count.to_i32
  end

  private def self.run_command(cmd : String, args : Array(String)) : NamedTuple(success: Bool, stdout: String, stderr: String)
    stdout = IO::Memory.new
    stderr = IO::Memory.new
    begin
      status = Process.run(cmd, args, output: stdout, error: stderr)
      {success: status.success?, stdout: stdout.to_s, stderr: stderr.to_s}
    rescue
      {success: false, stdout: "", stderr: "command not found"}
    end
  end
end
