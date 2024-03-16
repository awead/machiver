FROM ruby

RUN gem install exif pry bagit standardrb rspec

RUN mkdir /data
WORKDIR /data

COPY script/archive-photo.rb /usr/local/bin/archive-photo
RUN chmod 755 /usr/local/bin/archive-photo

COPY script/dedup-bag.rb /usr/local/bin/dedup-bag
RUN chmod 755 /usr/local/bin/dedup-bag

