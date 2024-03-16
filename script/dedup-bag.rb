#!/usr/local/bin/ruby

require "bagit"
require "optparse"
require "pathname"
require "pry"

def options
  @options ||= OpenStruct.new
end

ARGV.options do |opts|
  opts.banner = "Usage: dedup-bag [options] [bag_dir]"

  opts.on("-x", "--run", "Delete the files. Without specifying this flag, no files will be deleted, and duplicates will only be reported.") { |val| options.run = true }
  opts.on("-m", "--remote-manifest=", String, "Specify a manifest file to use for comparison.") { |val| options.remote_manifest = val }

  opts.on_tail("-h", "--help") do
    puts opts
    exit
  end

  opts.parse!
end

class BagDeduper
  attr_reader :bag, :options

  def initialize(path:, options:)
    @bag = BagIt::Bag.new(path)
    @options = options
  end

  def call
    set.each do |key, files|
      next if files.count < 2

      files.shift
      files.map do |file|
        if options.run
          remove_file(file)
        else
          puts "delete #{file}"
        end
      end
    end
  end

  private

  def manifest_md5
    @manifest_md5 ||= Pathname.new(bag.manifest_file("md5"))
      .readlines
      .map(&:chomp)
  end

  ##
  # The value of each hash key is an array of file paths. If a duplicate file exists, it is added to the array so that
  # any files that are present after the first one is identified can be deleted.
  #
  # @return [Hash]
  def set
    {}.tap do |set|
      manifest_md5.map do |manifest|
        hash, path = manifest.split
        set.has_key?(hash) ? set[hash].push(path) : set[hash] = [path]
      end
    end
  end

  def remove_file(relative_path)
    path = File.join(bag.bag_dir, relative_path)
    raise "Bag file does not exist: #{path}" unless File.exist? path
    FileUtils.rm path
  end
end

class RemoteDeduper < BagDeduper
  private

  def remote_manifest_md5
    @remote_manifest_md5 ||= Pathname.new(options.remote_manifest)
      .readlines
      .map(&:chomp)
  end

  ##
  # Uses the same process as the parent set method, except if duplicate file is found in the remote manifest, its path
  # is _prepended_ to the array so that any local duplicates will be slated for removal.
  #
  # @return [Hash]
  def set
    local = super

    remote_manifest_md5.map do |remote_manifest|
      hash, path = remote_manifest.split
      local[hash].prepend(path) if local.has_key?(hash)
    end

    local
  end
end

if options.remote_manifest
  RemoteDeduper.new(path: ARGV[0], options: options).call
else
  BagDeduper.new(path: ARGV[0], options: options).call
end
