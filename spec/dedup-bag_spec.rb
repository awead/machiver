RSpec.describe "Deduplicating bags" do
  context "when there are no duplicate files" do
    subject { `ruby script/dedup-bag.rb fixtures/good-bag` }

    it { is_expected.to eq("") }
  end

  context "when there are duplicate files in the bag" do
    subject { `ruby script/dedup-bag.rb fixtures/dup-bag`.split("\n") }

    it { is_expected.to contain_exactly("delete data/c.txt", "delete data/d.txt") }
  end

  context "when specifying a remote manifest with duplicates" do
    subject { `ruby script/dedup-bag.rb -m fixtures/remote-manifest-md5.txt fixtures/good-bag`.split("\n") }

    it { is_expected.to contain_exactly("delete data/b.txt") }
  end

  context "when there are both duplicates in the bag and in the remote manifest" do
    subject { `ruby script/dedup-bag.rb -m fixtures/remote-manifest-md5.txt fixtures/dup-bag`.split("\n") }

    it { is_expected.to contain_exactly("delete data/b.txt", "delete data/c.txt", "delete data/d.txt") }
  end
end
