require "./spec_helper"

describe Meta do
  describe ".dispatch" do
    it "returns false for empty args" do
      Meta.dispatch([] of String).should be_false
    end

    it "returns false for non-meta command" do
      Meta.dispatch(["status"]).should be_false
    end

    it "returns false for command starting with meta-like prefix" do
      Meta.dispatch(["metadata"]).should be_false
    end
  end

  describe ".help" do
    it "outputs help with fit and git versions" do
      output = IO::Memory.new
      Meta.help(output)

      help_text = output.to_s
      help_text.should contain("fit v#{Fit::VERSION}")
      help_text.should contain("(git ")
      help_text.should contain("USAGE:")
      help_text.should contain("OPTIONS:")
      help_text.should contain("COMMANDS:")
      help_text.should contain("META SUBCOMMANDS:")
      help_text.should contain("EXAMPLES:")
    end
  end
end

describe Meta::Doctor do
  describe ".fit_info" do
    it "returns installed tool info" do
      info = Meta::Doctor.fit_info
      info.installed.should be_true
      info.name.should eq("fit")
      info.version.should eq(Fit::VERSION)
    end
  end

  describe ".git_info" do
    it "detects git installation" do
      info = Meta::Doctor.git_info
      info.installed.should be_true
      info.name.should eq("git")
      info.version.should_not be_nil
    end
  end

  describe ".cpu_count" do
    it "returns positive integer" do
      Meta::Doctor.cpu_count.should be > 0
    end
  end

  describe ".os_name" do
    it "returns OS name" do
      os = Meta::Doctor.os_name
      os.should_not eq("unknown")
    end
  end

  describe ".shell" do
    it "returns shell path or unknown" do
      shell = Meta::Doctor.shell
      # Returns SHELL env var if set, "unknown" otherwise (e.g., in CI)
      shell.should be_a(String)
    end
  end

  describe ".git_addons" do
    it "returns an array of tool info" do
      addons = Meta::Doctor.git_addons
      addons.should be_a(Array(Meta::Doctor::ToolInfo))
      addons.size.should be > 0
    end
  end
end
