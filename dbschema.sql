--run this to create the database table we will be using
CREATE TABLE IF NOT EXISTS `HexDataStorage` (
    Pattern VARCHAR(256) DEFAULT '' COMMENT 'the pattern to lookup db info',
    Data MEDIUMBLOB COMMENT 'the NBT data of the object',
    Password TINYBLOB COMMENT 'the key to delete this data',
    Deletion TIMESTAMP DEFAULT NULL COMMENT 'The time when this was create to scheduel destruction',
    PRIMARY KEY (Pattern)
);

--delete all objects where the deletion time has passed
DELETE FROM `HexDataStorage` WHERE `Deletion` < NOW();
