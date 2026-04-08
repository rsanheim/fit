RSpec.describe "git-all status" do
  context "clean repositories" do
    it "shows clean for a repo with no changes and no tracking branch" do
      create_repo("my-repo")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "my-repo")
      expect(row).not_to be_nil
      expect(row.branch).to eq("main")
      expect(row.message).to eq("clean")
    end

    it "shows clean for a repo with a tracking branch" do
      with_tracking_repo("tracked-repo") do |_repo, _upstream|
        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "tracked-repo")
        expect(row).not_to be_nil
        expect(row.branch).to eq("main")
        expect(row.message).to eq("clean")
      end
    end

    it "shows HEAD (detached) for detached HEAD" do
      repo = create_repo("detached-repo")
      detach_head(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "detached-repo")
      expect(row).not_to be_nil
      expect(row.branch).to eq("HEAD (detached)")
      expect(row.message).to eq("clean")
    end
  end

  context "unstaged modifications" do
    it "shows 1 modified for one unstaged modification" do
      repo = create_repo("mod-repo")
      add_modified_file(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "mod-repo")
      expect(row.branch).to eq("main")
      expect(row.message).to eq("1 modified")
    end

    it "shows 3 modified for multiple unstaged modifications" do
      repo = create_repo("multi-mod-repo")
      add_modified_file(repo, "a.txt")
      add_modified_file(repo, "b.txt")
      add_modified_file(repo, "c.txt")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "multi-mod-repo")
      expect(row.message).to eq("3 modified")
    end
  end

  context "staged modifications" do
    it "shows 1 modified for one staged modification" do
      repo = create_repo("staged-mod-repo")
      add_staged_modification(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "staged-mod-repo")
      expect(row.message).to eq("1 modified")
    end

    it "counts staged + unstaged mod on same file once (MM)" do
      repo = create_repo("mm-repo")
      filepath = File.join(repo, "both.txt")
      File.write(filepath, "original\n")
      git(repo, "add", "both.txt")
      git(repo, "commit", "-m", "Add both.txt")
      File.write(filepath, "staged change\n")
      git(repo, "add", "both.txt")
      File.write(filepath, "unstaged change on top\n")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "mm-repo")
      expect(row.message).to eq("1 modified")
    end
  end

  context "staged new files" do
    it "shows 1 added for one staged new file" do
      repo = create_repo("added-repo")
      add_staged_new_file(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "added-repo")
      expect(row.message).to eq("1 added")
    end

    it "counts staged add with worktree mod as added (AM)" do
      repo = create_repo("am-repo")
      filepath = File.join(repo, "am-file.txt")
      File.write(filepath, "new file\n")
      git(repo, "add", "am-file.txt")
      File.write(filepath, "modified after staging\n")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "am-repo")
      expect(row.message).to eq("1 added")
    end
  end

  context "deletions" do
    it "shows 1 deleted for one staged deletion" do
      repo = create_repo("staged-del-repo")
      stage_deletion(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "staged-del-repo")
      expect(row.message).to eq("1 deleted")
    end

    it "shows 1 deleted for one unstaged deletion" do
      repo = create_repo("unstaged-del-repo")
      unstaged_deletion(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "unstaged-del-repo")
      expect(row.message).to eq("1 deleted")
    end
  end

  context "renames" do
    it "shows 1 renamed for a rename" do
      repo = create_repo("rename-repo")
      stage_rename(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "rename-repo")
      expect(row.message).to eq("1 renamed")
    end
  end

  context "untracked files" do
    it "shows 1 untracked for one untracked file" do
      repo = create_repo("untracked-repo")
      add_untracked_file(repo)

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "untracked-repo")
      expect(row.message).to eq("1 untracked")
    end

    it "shows 2 untracked for multiple untracked files" do
      repo = create_repo("multi-untracked-repo")
      add_untracked_file(repo, "a.txt")
      add_untracked_file(repo, "b.txt")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "multi-untracked-repo")
      expect(row.message).to eq("2 untracked")
    end
  end

  context "mixed changes" do
    it "shows modified + untracked" do
      repo = create_repo("mixed-repo")
      add_modified_file(repo, "mod.txt")
      add_untracked_file(repo, "new.txt")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "mixed-repo")
      expect(row.message).to eq("1 modified, 1 untracked")
    end

    it "shows all types present in correct order" do
      repo = create_repo("all-types-repo")

      # Commit all files we'll modify/delete/rename in a single commit
      File.write(File.join(repo, "mod.txt"), "original\n")
      File.write(File.join(repo, "del.txt"), "will be deleted\n")
      File.write(File.join(repo, "old.txt"), "will be renamed\n")
      git(repo, "add", "mod.txt", "del.txt", "old.txt")
      git(repo, "commit", "-m", "Add files")

      # Now create all states at once (no intermediate commits)
      File.write(File.join(repo, "mod.txt"), "modified\n")
      git(repo, "rm", "del.txt")
      git(repo, "mv", "old.txt", "new.txt")
      File.write(File.join(repo, "add.txt"), "new file\n")
      git(repo, "add", "add.txt")
      File.write(File.join(repo, "untracked.txt"), "untracked\n")

      result = run_git_all("status")
      rows = parse_output(result.stdout)

      row = find_repo(rows, "all-types-repo")
      expect(row.message).to eq("1 modified, 1 added, 1 deleted, 1 renamed, 1 untracked")
    end
  end

  context "ahead/behind tracking" do
    it "shows clean, 2 ahead when ahead of remote" do
      with_tracking_repo("ahead-repo") do |repo, _upstream|
        make_ahead(repo, 2)

        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "ahead-repo")
        expect(row.message).to eq("clean, 2 ahead")
      end
    end

    it "shows clean, 3 behind when behind remote" do
      with_tracking_repo("behind-repo") do |repo, upstream|
        make_behind(upstream, repo, 3)

        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "behind-repo")
        expect(row.message).to eq("clean, 3 behind")
      end
    end

    it "shows clean, 2 ahead, 3 behind when diverged" do
      with_tracking_repo("diverged-repo") do |repo, upstream|
        make_ahead(repo, 2)
        make_behind(upstream, repo, 3)

        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "diverged-repo")
        expect(row.message).to eq("clean, 2 ahead, 3 behind")
      end
    end

    it "shows 1 modified, 1 ahead when modified and ahead" do
      with_tracking_repo("mod-ahead-repo") do |repo, _upstream|
        make_ahead(repo, 1)
        File.write(File.join(repo, "README.md"), "modified\n")

        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "mod-ahead-repo")
        expect(row.message).to eq("1 modified, 1 ahead")
      end
    end

    it "shows mixed changes + diverged" do
      with_tracking_repo("mixed-diverged-repo") do |repo, upstream|
        make_ahead(repo, 2)
        make_behind(upstream, repo, 1)
        File.write(File.join(repo, "README.md"), "modified\n")
        add_untracked_file(repo, "b.txt")

        result = run_git_all("status")
        rows = parse_output(result.stdout)

        row = find_repo(rows, "mixed-diverged-repo")
        expect(row.message).to eq("1 modified, 1 untracked, 2 ahead, 1 behind")
      end
    end
  end

end
