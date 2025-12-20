require "../spec_helper"

describe Commands::Fetch do
  subject = Commands::Fetch.new

  describe "#git_args" do
    it "returns fetch args" do
      args = subject.git_args("/path/to/repo")
      args.should eq(["fetch"])
    end
  end

  describe "#format_output" do
    it "returns error message on failure" do
      result = subject.format_output("", "fatal: not a git repository", false)
      result.should eq("fatal: not a git repository")
    end

    it "returns 'no new commits' for empty output" do
      result = subject.format_output("", "", true)
      result.should eq("no new commits")
    end

    it "returns 'no new commits' when only From line in stderr" do
      result = subject.format_output("", "From github.com:user/repo", true)
      result.should eq("no new commits")
    end

    it "counts single branch update" do
      stdout = "   abc123..def456  main       -> origin/main\n"
      result = subject.format_output(stdout, "", true)
      result.should eq("1 branch updated")
    end

    it "counts multiple branch updates" do
      stdout = <<-OUTPUT
         abc123..def456  main       -> origin/main
         111222..333444  develop    -> origin/develop
      OUTPUT
      result = subject.format_output(stdout, "", true)
      result.should eq("2 branches updated")
    end

    it "counts single tag" do
      stdout = " * [new tag]         v1.0.0     -> v1.0.0\n"
      result = subject.format_output(stdout, "", true)
      result.should eq("1 tag updated")
    end

    it "counts multiple tags" do
      stdout = <<-OUTPUT
       * [new tag]         v1.0.0     -> v1.0.0
       * [new tag]         v1.0.1     -> v1.0.1
      OUTPUT
      result = subject.format_output(stdout, "", true)
      result.should eq("2 tags updated")
    end

    it "counts mixed branches and tags" do
      stdout = <<-OUTPUT
         abc123..def456  main       -> origin/main
       * [new tag]         v1.0.0     -> v1.0.0
      OUTPUT
      result = subject.format_output(stdout, "", true)
      result.should eq("1 branch, 1 tag updated")
    end

    it "returns 'fetched' when output exists but no update lines" do
      stdout = "some other output\n"
      result = subject.format_output(stdout, "", true)
      result.should eq("fetched")
    end
  end
end
