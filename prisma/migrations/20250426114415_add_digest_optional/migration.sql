-- AlterTable
ALTER TABLE "displays" ALTER COLUMN "last_tx_digist" DROP NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "objects" ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();

-- AlterTable
ALTER TABLE "packages" ALTER COLUMN "last_tx_digist" DROP NOT NULL,
ALTER COLUMN "updated_at" SET DEFAULT NOW(),
ALTER COLUMN "created_at" SET DEFAULT NOW();
