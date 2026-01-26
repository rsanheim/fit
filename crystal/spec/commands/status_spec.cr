require "../spec_helper"

describe Commands::Status do
  subject = Commands::Status.new

  describe "#git_args" do
    it "returns porcelain status args" do
      args = subject.git_args("/path/to/repo")
      args.should eq(["status", "--porcelain"])
    end
  end

  describe "#format_output" do
    it "returns 'clean' for empty output" do
      subject.format_output("", "", true).should eq("clean")
    end

    it "counts modified files" do
      output = " M file1.txt\n M file2.txt\n"
      subject.format_output(output, "", true).should eq("2 modified")
    end

    it "counts untracked files" do
      output = "?? new-file.txt\n?? another.txt\n"
      subject.format_output(output, "", true).should eq("2 untracked")
    end

    it "counts added files" do
      output = "A  staged-file.txt\n"
      subject.format_output(output, "", true).should eq("1 added")
    end

    it "counts deleted files" do
      output = "D  deleted.txt\n"
      subject.format_output(output, "", true).should eq("1 deleted")
    end

    it "combines multiple statuses" do
      output = " M modified.txt\n?? untracked.txt\nA  added.txt\n"
      result = subject.format_output(output, "", true)
      result.should contain("1 modified")
      result.should contain("1 added")
      result.should contain("1 untracked")
    end

    it "returns error message on failure" do
      result = subject.format_output("", "fatal: not a git repository", false)
      result.should eq("fatal: not a git repository")
    end
  end
end
